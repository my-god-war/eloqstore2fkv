# 27b P1：读路径持 mapper 快照 + index 缓存代际

父 issue：`27-no-cow-read-snapshot.md`。

## 背景

读 / scan 直接借当前 `&PageMapper`：

- `crate/engine/fkv_task_read/src/lib.rs:108`（`read_entry`）、
  `:191`（`read_floor_entry`）、`:82`（`descend`）、`:20`（`read_overflow`）
- `crate/engine/fkv_task_scan/src/lib.rs:59`（`ScanArgs.mapper`）
- `crate/engine/fkv_store/src/lib.rs:665`（`get_at`）→ `:534`（`read_ctx`）

写路径 `remap_page` 原地改 `MappingTbl`（`crate/core/fkv_mapping/src/lib.rs:771`），
写并发后读会看到写中途状态。仅加 mapper 快照仍不够：`TableState.index_cache`
是表级共享的（`fkv_store/src/lib.rs:126`），旧快照读者会命中新版本 `IndexNode`。

## 目标

1. 读入口从 `&PageMapper` 改为 `&MappingSnapshot`（由 `27a` 定义），一个请求
   取一次快照，整个点读 / scan 复用；`root_id` 等也随快照固定。
2. `IndexCache` 增加 mapping 代际（或快照 id）键：`get` / `insert` 必须匹配
   当前代，旧快照命中新代缓存必须 miss。实现二选一：
   - 缓存项带代际，`get` 比对；
   - 写提交时清空缓存，并有活跃快照时读路径禁用缓存（保守，先过验收）。
3. 一次改净 `read_entry` / `read_floor_entry` / `read_overflow` / scan /
   `get` / `get_large` / `floor` 的签名，**禁止双接口并存**。
4. `Store` 的 `read_ctx` 同步改持快照引用，不留下「快照 + 当前 mapper」混用。

## 约束

- 不得把 `IndexCache` 改成锁或 `Arc<Mutex>`；shard 单线程。
- 不得为了兼容保留旧 `&PageMapper` 入口。
- 快照 drop 后再允许物理页回池；与 `27a` registry 生命周期一致。

## 验收

- 所有读 / scan 调用点只接受 `MappingSnapshot`，编译期杜绝误用。
- 读在 `io.read_page` await 点挂起、并发 `put` + `remap_page` 后，结果等于
  读起点的快照；索引缓存不返回新代节点。
- `cargo nextest run -p fkv_task_read -p fkv_task_scan -p fkv_store`；
  clippy 零警告。

## 依赖

- `27a` 的 `MappingSnapshot` / registry 类型。
- `eng-32b` 提供真正的写读并发场景；在此之前只能单测快照语义。

## DONE（2026-11-05）

### 状态

27b 最小闭环完成：读 / scan 全链路一次请求持一份 `MappingSnapshot`，
`IndexCache` 按映射代际隔离，旧快照不命中新代节点；无双接口并存。

### 改动

- `crate/core/fkv_mapping/src/lib.rs`
  - `MappingTbl::file_page_id`：快照直接按逻辑页取物理页。
  - `PageMapper` 新增 `generation`：`allocate_page` / `remap_page` /
    `set_file_page` / `free_page` 递增；`BatchJournal` 记录并在 `abort_batch`
    回退，失败批次后的旧缓存重新有效。
  - `PageMapper::read_view`：segment 侧只读视图（`fkv_large` 读接口仍收
    `&PageMapper`，不扩散改造到 fkv_large）。
  - `SnapshotCore` 新增 `mapping_generation`（取 `source.data.generation()`）；
    `MappingSnapshot::{mapping_generation, segment_mapper}`；segment 侧改存
    `Option<PageMapper>` 视图，`segment_tbl` 无 segment 时返回共享空表。
- `crate/core/fkv_btree/src/lib.rs`（`IndexCache` 所在地，按目标 2 必要改动）
  - `CacheEntry` 带 `generation`；`get(page_id, generation)` 代际不符即 miss；
    `insert(page_id, node, generation)` 记录代际，跨代回填计一次 miss。
  - `retag(generation)`：写提交 / reopen 后把未变更条目重标到当前代。
  - 单测：`generation_gates_hits_and_retag_refreshes`。
- `crate/engine/fkv_task_read/src/lib.rs`
  - `ReadCtx` 改持 `snapshot: &MappingSnapshot`（新增 `ReadCtx::new`，去掉
    `Default`）；`read_entry` / `read_floor_entry` 去掉 `root_id` / `&PageMapper`
    参数，root 取自快照。
  - `read_overflow` / `descend_step` / `page_floor` / `materialize` 全部改收
    `&MappingSnapshot`；`page_cnt` 用快照映射长度。
- `crate/engine/fkv_task_scan/src/lib.rs`
  - `ScanArgs` 去掉 `root_id` / `mapper`，改 `snapshot: &MappingSnapshot` +
    `ScanTree::{Data, Ttl}`（TTL 树 root 亦由快照固定）。
- `crate/engine/fkv_store/src/lib.rs`
  - `get_at` / `read_entry_at` / `resolve_value` / `floor` / `get_large*` /
    `scan_entry_page_at` / `scan_entry_opts` / `truncate_from` / `clean_expired`
    全部在入口取一次 `Store::snapshot` 并在整个请求复用；large value 的
    segment 解析也用同一快照的 segment 视图。
  - `read_ctx` / `read_ctx_at` 改持快照引用，不再混用当前 `PageMapper`。
  - `invalidate_index_cache` 精确移除变更页后 `retag` 到当前 mapper 代际；
    `compact_file_li` 成功后同样 retag；`install_replay_state` 在
    `retain_mapped` 后把保留条目重标到新 mapper 代际，保持 reopen 复用。
- 测试
  - `crate/engine/fkv_task_read/tests/read.rs`：新增
    `read_suspended_at_io_uses_start_snapshot`（可控 IO 在首个 `read_page`
    挂起，期间 `remap_page` + 写新值，恢复后仍返回旧值）与
    `old_snapshot_misses_new_generation_cache_node`（新代缓存节点不被旧快照
    命中）；全部调用点改显式快照。
  - `crate/engine/fkv_task_scan/tests/scan.rs`、`fkv_task_write/tests/write.rs`
    调用点改 `snapshot` / `ScanTree`。
  - `crate/engine/fkv_store/tests/index_cache.rs`：新增
    `write_retags_unmodified_index_pages_for_next_reads`，断言写后未变更索引页
    仍可命中（防止代际改造退化成每写清空缓存）。

### 语义与取舍

- 代际是 `PageMapper` 的映射版本，不是快照创建序号：同一映射代内多请求共享
  缓存；任何映射变更（含 compact 换物理页）使旧快照 miss。
- 写提交后 `retag` 只覆盖「变更页已精确移除」后的存活条目，未变更子树继续
  命中；旧快照因代际较小仍不会命中这些条目。`abort_batch` 回退代际，失败
  批次不会让旧缓存失效。
- 读请求每次 `Store::snapshot` 会克隆 chunk 指针（O(chunks)，不复制 4 KiB
  块）并登记弱引用；高频读的额外开销尚未做快照池化优化。

### 验证

- `cargo nextest run -p fkv_mapping -p fkv_btree -p fkv_task_read -p fkv_task_scan
  -p fkv_task_write -p fkv_store --no-fail-fast`：216 passed。
- `cargo clippy -p fkv_mapping -p fkv_btree -p fkv_task_read -p fkv_task_scan
  -p fkv_task_write -p fkv_store --all-targets --all-features -- -D warnings`：
  0 warning。
- `cargo check -p fkv_shard -p fkv_sdk -p fkv_runtime -p fkv_task_gc`：通过。
- 验证用独立 `CARGO_TARGET_DIR`，避免与并发子代理/worktree 争用同一 target
  导致指纹串扰。

### 剩余项 / 风险

- 27c 的 shard 级真实「读挂起 + 并发写」验收仍需 `eng-32b`（每请求 spawn /
  跨 await 读写）落地；本轮在任务层用可控 IO 模拟并通过。
- 快照注册表按请求登记，读放大时 `Store::snapshot` 的 chunk 指针克隆与弱引用
  入表是 O(chunks)+O(live) 开销；如需进一步降本可加快照复用池（非本单验收）。
- `MappingTbl::file_page_id` 与 `PageMapper::file_page_id` 共用同一活跃槽位
  判定；`MappingTbl::set` 仍仅供 mapper 内部使用。

## 复审（2026-11-05 r6）

通过。逐项亲读核验：

- `fkv_mapping/src/lib.rs:955`（`MappingSnapshot`）、`:982`（`mapping_generation`）、
  `:1009`（`segment_mapper` 只读视图）、`:619`（`PageMapper::read_view`）、
  `:627`（`generation`，`abort_batch` 回退见 journal :699 一带）。
- `fkv_btree/src/lib.rs:290-298`（`get(page_id, generation)` 代际不符即 miss，不驱逐）、
  `:300-340`（`insert` 记录代际、跨代回填计一次 miss）、`:432`（`retag`）；
  测试 `:1062`（`generation_gates_hits_and_retag_refreshes`）存在。
- `fkv_task_read/src/lib.rs:172-191`（`ReadCtx { snapshot, .. }` + `ReadCtx::new`）、
  `:224-228`（下降代际校验）；`fkv_task_scan/src/lib.rs:74`（`ScanArgs.snapshot`）、
  `:176`（叶子链上界用快照映射长度）。
- `fkv_store/src/lib.rs:1171`（`Store::snapshot`）、`:637`（`invalidate_index_cache`
  按当前代际失效）、`:1810/:2358`（reopen / compact 后 `retag`）。
- 配套测试亲读：`fkv_store/tests/index_cache.rs:255-261`（写后未变更子树缓存仍命中，
  断言 reads 上界）。

遗留记录在案：27c 并发真实验收等 eng-32b；`Store::snapshot` 每请求 chunk 指针克隆
未池化；`PageMapper::page_cnt()`（`fkv_mapping/src/lib.rs:871`）在快照化改用
`snapshot.data_tbl().len()` 后全仓零调用，成为孤儿 pub API（归入 lane 报告遗留风险，
fkv_task_read/scan 为并行热点，清理需与 owner 协调）。
