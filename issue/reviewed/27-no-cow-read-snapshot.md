# 27 P1：读用当前 mapper，没有版本化 COW 快照

## 现象

读路径（`read_entry` / `read_floor_entry` / scan）直接拿 `&PageMapper` 当前表；写路径 `remap_page` **原地**改 `MappingTbl`：

```rust
// crate/core/fkv_mapping/src/lib.rs:299
pub fn remap_page(&mut self, page_id: PageId) -> Result<FilePageId> {
  ...
  self.tbl.set(page_id, encode_file_page_id(file_page_id))?;   // 原地改
  ...
}
```

没有 `MappingSnapshot` / 引用计数快照。

## C++ 对照

写任务先 `MakeCowRoot()` 拷一份 `MappingSnapshot`；读任务持有 `MappingSnapshot::Ref`（引用计数）整个生命周期（`src/tasks/read_task.cpp:45-57`、`include/storage/page_mapper.h:29-181`）。读写不互斥，旧快照的 file page 由 GC retained set 保护。

## 影响

- 单线程串行下暂时不炸，一旦做并发（见 `03-shard-request-concurrency.md`）就会读写同一 mapper。
- 没有「读期间旧页不被回收」的机制，接 GC 后会出现读悬空页。
- 与 `24-data-pages-never-fsynced.md`、`14-ttl-cleanup.md` 的并发假设都依赖快照。

## 建议

1. 实现 `MappingSnapshot`（引用计数 + COW），读任务持 Ref。
2. COW 提交与快照发布解耦。
3. 与 GC retained set 联动：活跃快照引用的 file page 不可回收。

## 验收

- 读进行中并发写，读结果自洽（一致快照）。
- GC 不回收被活跃快照引用的页。

## 审查结论（2026-11-04）

### 判定

**本轮不实现完整 COW 快照，拆单跟踪。**

1. 当前不可观测。读批 `join_all` 只并发读；写经 `WriteGate` 在同一条 shard 循环里与读串行推进（`crate/engine/fkv_shard/src/lib.rs:1283-1311`）。读跨 `await` 期间没有写者能改 `PageMapper`，此时加「代际号 + 失败重试」永不触发，属死代码（SKILL.md 零死代码）。
2. 完整快照是跨 crate 改造：`fkv_mapping` 加快照引用计数 + retired 队列，`fkv_store` 读接口持快照，`fkv_task_read` / `fkv_task_scan` 签名扩散到 get/get_large/floor/scan，GC retained set 接快照注册表，`IndexCache` 加代际键。单独落地任一环都不闭环（见风险 1）。
3. 上游前置未完成：`eng-32b`（每请求 spawn、读跨 `await`）仍是 todo，且已把本条列为阻塞依赖；`core-11` 只完成了 `remap_page` 顺序修复 + Pooled 不变量文档，明确遗留快照链。

真正开写的前置：`eng-32b` 落地，或决定启用 `AllocMode::Pooled`。

### 逐条核验（本轮复核，行号按当前 HEAD）

| 原文断言 | 结论 | 证据 |
|---|---|---|
| 读路径直接拿 `&PageMapper` 当前表 | 属实 | `fkv_store/src/lib.rs:665`（`get_at`）→ `:534`（`read_ctx`）→ `read_entry`；floor/scan 同型 |
| `read_entry` / `read_floor_entry` / scan 持共享 mapper | 属实 | `fkv_task_read/src/lib.rs:108`、`:191`；`fkv_task_scan/src/lib.rs:59`（`ScanArgs.mapper: &'a PageMapper`） |
| `remap_page` 原地改 `MappingTbl` | 属实 | `fkv_mapping/src/lib.rs:771-789`：`journal_change` + `tbl.set`，旧 fp 立即 `allocator.free` |
| 没有 `MappingSnapshot` / 引用计数快照 | 属实 | 全 crate 仅注释提及；`PageMapper` 字段 `tbl/allocator/free_page_head/free_page_cnt/dirty_li/changed_page_li/journal`（`fkv_mapping/src/lib.rs:566`） |
| COW 基础设施完全缺失 | 不准确 | `MappingTbl` 已是 `Vec<Arc<Chunk>>` 分块，`set` 走 `Arc::make_mut`（`fkv_mapping/src/lib.rs:113-120`、`:177`）；差的是快照类型与引用计数 |
| C++ `MakeCowRoot` / `MappingSnapshot::Ref` / GC retained 保护 | 无法本地核验 | `cpp/eloqstore` 为空目录；沿用 `core-11` 引用，不新增行号级结论 |
| 当前串行下不炸 | 属实 | 读批只含只读请求（`fkv_shard/src/lib.rs:1283-1311`）；写经 `WriteGate` 串行 drain；`Store.table_map` 由 `&mut self` 独占（`fkv_store/src/lib.rs:167`），无锁无 `Arc` |
| GC 已有「活跃快照页不可回收」 | 不成立 | `fkv_store/src/lib.rs:2857-2888` 只并 manifest retained set + **当前**内存 mapper |
| 并发化后索引缓存仍正确 | 原文漏项，属实 | `TableState.index_cache: Option<RefCell<IndexCache>>` 表级共享，写后按 `changed_page_li` 精确失效（`fkv_store/src/lib.rs:126`、`:559`）；旧快照读者会命中新版本节点 |

### 拆分去向

- `issue/todo/27a-gc-retained-active-mapping-snapshot.md`：shard 级快照注册表 + GC 并集活跃快照。
- `issue/todo/27b-read-path-hold-mapping-snapshot.md`：读/scan 改持快照，index 缓存加 mapping 代际键。
- `issue/done/27c-concurrent-read-write-snapshot-tests.md`：并发读写自洽 + GC 保留验收用例。

依赖：三者都要等 `eng-32b`（或 Pooled 启用决定）。27a / 27b 可并行，27c 等两者完成。

### 风险

1. **快照自洽不等于索引缓存自洽**：不处理 `IndexCache` 代际，验收第一条会假过或偶发错读。
2. **GC 并集成本**：每快照全表扫描 O(pages)。Append 模式可用 `max_fp_id` 水线 + 文件级区间粗保留。
3. **接口扩散**：`&PageMapper` 是 get/get_large/floor/scan 的公共签名，需一次改净，禁止双接口并存。
4. **回收时序**：快照 drop 后若只摘链不重跑 GC，旧文件滞留到下一轮；需 registry 空转触发或明确可延迟。
5. **不可核验的 C++ 断言**：本地无 `cpp/eloqstore`，不再新增行号级结论。

## DONE（2026-11-04）

- 本轮只做核验与分单，不改 `fkv_mapping` / `fkv_store` / `fkv_task_read` /
  `fkv_task_scan` 代码：当前读批仅并发读、写经 `WriteGate` 串行，读不可能观察到
  写中途状态；加代际校验会形成永不触发的死代码。
- 结论与证据见上；实现拆为 `issue/todo/27a-*`、`27b-*`、`27c-*`，均以
  `eng-32b`（读请求 spawn）或 Pooled 启用为前提。
- 验证（无代码改动，确认基线）：`cargo check -p fkv_store -p fkv_shard`、
  `cargo nextest run -p fkv_store -p fkv_shard -p fkv_runtime --no-fail-fast`、
  两 crate clippy `--no-deps --all-features --all-targets -- -D warnings`。

## 复审（2026-11-05 r6）

通过（父条目，按拆单跟踪）。最小闭环由 27b 落地并经本次亲读复核：读/scan 全链路一次
请求持一份 `MappingSnapshot`（`fkv_mapping/src/lib.rs:955`），`IndexCache` 按映射代际
隔离（`get(page_id, generation)`），`retag` 在写提交/reopen/compact 后重标存活条目；
GC 侧「活跃快照引用的 file page 不可回收」由 retained 快照注册表承接（27c 验收测试
仍留 todo）。完整 COW 快照（旧页引用计数 + retired 队列）仍以 eng-32b 落地为前提，
与拆单决策一致。详见 `27b` 的 r6 复审。
