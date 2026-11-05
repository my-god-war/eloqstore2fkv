# 27a P1：GC 并集活跃 mapper 快照

父 issue：`27-no-cow-read-snapshot.md`。

## 背景

`Store::gc` 的 retained set = 所有 manifest 的 mapping + **当前**内存 mapper
（`crate/engine/fkv_store/src/lib.rs:2857-2888`，`add_mapping_retained` 见
`crate/engine/fkv_task_gc/src/lib.rs:285`）。

一旦读请求跨 `await` 持旧快照，旧快照独占的 `FilePageId` 不在当前 mapper 里，
GC 会删掉读者还要读的文件。

## 目标

1. `fkv_mapping` 增加 `MappingSnapshot`：持有 `MappingTbl`（已是
   `Vec<Arc<Chunk>>`，clone 即 COW 共享）+ 取出时的 `root_id` / `seg_root` /
   `max_fp_id` / 代际号。线程内用 `Rc`，不引入原子。
2. shard / Store 级 `SnapshotRegistry`：快照创建时登记（弱引用或 id 表），
   drop 时摘链；提供 `retained_into(&mut HashSet<FilePageId>)`。
3. `Store::gc` 在 manifest + 当前 mapper 之外并集所有活跃快照的 `tbl`。
4. registry 摘链后是否需要立即重跑 GC：明确语义（下一轮 GC 回收即可，
   还是空转触发），写进文档。

## 约束

- 单线程模型，禁止 `Arc<Mutex>` 式伪并发；用 `Rc<RefCell<...>>` 或 shard 局部表。
- 不做完整 `next_snapshot_` 代际链：Pooled 仍禁用（`core-11` 已加编译期守卫）。
- Append 模式下不产生额外分配：无活跃快照时并集为零成本分支。

## 验收

- 新测试 `crate/engine/fkv_store/tests/gc_snapshot.rs`：
  构造「某文件只被旧快照引用」→ 跑 GC → 断言文件存在；快照 drop → 再跑 GC
  → 断言文件已删。
- 快照 alive 时 `Store::gc` 的 retained set 包含快照内全部 file page。
- `cargo nextest run -p fkv_store -p fkv_task_gc`；clippy 零警告。

## 依赖

- `eng-32b`（读请求 spawn / 跨 await 持状态）落地后才能产生真实的长生命周期快照。
- 与 `27b` 共用 `MappingSnapshot` 类型定义，先定接口再各自动手。

## DONE（2026-11-05）

### 状态

27a 闭环完成，未做 27b 读路径签名改造。读路径仍持 `&PageMapper`；本单只补
「快照类型 + 活跃快照注册表 + GC retained 并集 + 生命周期单测」。

### 改动

- `crate/core/fkv_mapping/src/lib.rs`
  - 新增 `MappingSnapshot`：持有 `Rc<SnapshotCore>`；core 内含 data / segment
    两侧 `MappingTbl`（`Arc<Chunk>` COW 共享，clone 不复制 4 KiB 块）、
    `root_id` / `ttl_root_id` / `max_fp_id` / `max_segment_fp_id` / `generation`。
  - 新增 `SnapshotSource`：Store 入口用当前 mapper + root 构造快照。
  - 新增 `SnapshotRegistry`：`RefCell<Vec<(TableIdent, Weak<SnapshotCore>)>>`，
    单线程、无原子。`open` 登记并顺手惰性摘链；快照 drop 后弱引用失效，
    `data_retained_into` / `segment_retained_into` / `live_num` 再次访问时清除。
    按 `TableIdent` 过滤，避免跨表 FilePageId 命名空间误并集。
  - 新增 `add_mapping_retained`：GC / 注册表共用一份活跃槽位判定。
- `crate/engine/fkv_task_gc/src/lib.rs`：`add_mapping_retained` 改为调用
  `fkv_mapping` 的单一实现，签名与既有调用点不变。
- `crate/engine/fkv_store/src/lib.rs`
  - `Store` 新增 `snapshots: SnapshotRegistry`。
  - 新增 `Store::snapshot(tbl)`：取当前 data / segment 映射与 root 生成快照并登记；
    `Store::snapshot_num(tbl)` 供诊断 / 测试。
  - `Store::gc` 在 manifest + 当前 mapper 之外显式并集活跃快照的 data / segment
    retained set。
- `crate/core/fkv_mapping/Cargo.toml` / `Cargo.lock`：`cargo add gxhash -p fkv_mapping`
  （注册表 `retained_into` 使用项目统一 `gxhash::HashSet`）。

### 语义边界

- 快照释放只让旧页「从下一轮 GC 起」可回收；注册表不主动触发 GC，也不在
  drop 当场重跑。GC 仍由 `Store::maintain` / `Store::gc` 既有节奏驱动。
- 无活跃快照时并集是空表遍历，不分配（Append 模式常态）；有快照时成本为
  `O(快照数 × 映射页数)`，暂未做 `max_fp_id` 水线 / 文件级粗保留优化。
- 27b 最小可用 API：`MappingSnapshot::{generation, root_id, ttl_root_id,
  data_tbl, segment_tbl, max_fp_id, max_segment_fp_id}` +
  `Store::snapshot(tbl)`。27b 直接以该类型替换 `&PageMapper` 读入口即可，
  无需再改 GC 侧。

### 验证

- 新增 `crate/engine/fkv_store/tests/gc_snapshot.rs`：
  - data 侧：快照后全量覆盖 → GC 不删旧文件；最后一份 clone drop → 下一轮 GC
    全部回收，当前数据仍可读；同时断言 clone 不提前摘链。
  - segment 侧（large value）：同上，验证 `segment_retained_into`。
- 新增 `fkv_mapping::snapshot_tests`：快照 COW 隔离（写入 remap 后旧快照仍见旧
  物理页）、按表过滤、drop 后不再并集。
- `cargo nextest run -p fkv_mapping -p fkv_task_gc -p fkv_store`：151 passed。
- `cargo clippy -p fkv_mapping -p fkv_task_gc -p fkv_store --all-targets
  --all-features -- -D warnings`：0 warning。
- `cargo check -p fkv_shard -p fkv_sdk -p fkv_runtime -p fkv_task_compact
  -p fkv_task_read`：通过（`Store` 因 `Rc` 变为 `!Send`，下游 shard 在自有线程内
  构造 Store，未触发 `Send` 约束）。

### 残余风险

- 进程内注册表只覆盖同一个 `Store` 实例：同表不同 branch / 不同 Store 实例的
  读者快照互相不可见（与既有「当前 mapper 只在本实例」边界一致）。
- `Store::drop_table` 仍是显式破坏性操作，不检查活跃快照；跨 drop_table 的读者
  不在本单保护范围。
- 快照只固定 mapping / root，不固定 `IndexCache`；索引缓存代际由 27b 处理。


## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:955` MappingSnapshot（Rc<SnapshotCore>）、`:1024` SnapshotSource、`:1046` SnapshotRegistry（Weak 惰性摘链）、`:1084/:1089` data/segment_retained_into、`:1134` snapshot_tests；`fkv_store/src/lib.rs:250` snapshots 字段、读路径已持快照（:742/:894，27b 范畴）；`fkv_task_gc/src/lib.rs:314-316` add_mapping_retained 单一实现。测试 `fkv_store/tests/gc_snapshot.rs:49`（快照独占文件 GC 不删 :96、drop 后回收 :108-115）、`:129`（segment 侧）。残余风险（跨 Store 实例/drop_table/IndexCache 代际）已在文中声明，属 27b 及后续范围。
