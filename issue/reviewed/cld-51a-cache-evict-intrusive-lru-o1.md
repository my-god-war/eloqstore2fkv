# cld-51a cache 淘汰候选改为全局侵入式 LRU 链表，victim 选取 O(1)

## 位置
- `crate/engine/fkv_cloud/src/lib.rs`
  - `oldest_clean_data` / `oldest_clean_segment`（全表 `min_by_key`）
  - `evict_candidate`（每淘汰一个 victim 重扫一遍）
  - `FileState.last_used` / `SegmentState.last_used` / `CacheState.clock`
  - `touch` / `touch_segment` / `upsert_resident` / `upsert_segment_resident` /
    `mark_clean` / `mark_segment_clean` / `mark_dirty_range` / `forget` /
    `forget_segment` / `evict_table`（所有 entry 状态变更点）

## 现状
- 超预算时每次 `evict_candidate()` 对 `table_map` 全量 `flat_map + filter(!dirty) +
  min_by_key(last_used)`；`evict_to_budget` 的 `while` 每淘汰一个就重扫，
  M 个 victim = M×O(N)。
- 未超预算已早退，不做扫描（事实修正，原 issue 已注明）。

## 方案（对齐 C++ `lru_file_head_/tail_`，`:3589-3590`、`:4230`）
- 在 `CacheState` 里维护**一条全局侵入式 LRU 双向链表**：
  - `node_li: Vec<LruNode>` 索引型 arena，`LruNode { prev, next, table_idx,
    file_id, segment }`；
  - `lru_head` 为新端、`lru_tail` 为旧端，victim 取 `lru_head` O(1)；
  - 空指针哨兵 `LRU_NIL = u32::MAX`，`node_li` 用空闲链复用（不泄漏、不增长）。
- 表名反查：`table_li: Vec<Option<TableIdent>>` + `free_table_li`，节点只存
  `table_idx`，**不为每个文件复制 TableIdent**。
- `FileState` / `SegmentState` 存 `lru_idx: u32`，删掉 `last_used`，`CacheState`
  删掉 `clock`（链表顺序即 LRU 顺序，无需时间戳）。
- dirty entry 不入链（对齐 C++ dirty 不进 `closed_files_` LRU）：
  - `upsert_*` 置脏时摘链、置净时入链尾；
  - `mark_dirty_range` 置脏时摘链；
  - `mark_clean` 入链尾；
  - `touch` / `touch_segment` 命中时挪到链尾；
  - `forget` / `forget_segment` / `evict_table` 摘链并回收节点。
- 删除 `oldest_clean_data` / `oldest_clean_segment` 两个全表扫描函数。

## 验收
- `cargo check -p fkv_cloud` 通过。
- `cargo nextest run -p fkv_cloud` 全绿，现有 LRU 语义测试不变：
  `cache_budget_evicts_and_redownloads_whole_files`（file 0 先淘汰）、
  `segment_cache_respects_budget_and_redownloads`、`evict_table_drops_only_that_table_cache`。
- 新增跨表 LRU 顺序测试：两表交替读写，超预算时淘汰全局最久未用文件的表。
- `cargo clippy -p fkv_cloud --no-deps --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

实现要点（`crate/engine/fkv_cloud/src/lib.rs`）：

1. `CacheState` 新增全局侵入式 LRU：
   `node_li: Vec<LruNode>`（索引 arena + 空闲链复用，`LRU_NIL = u32::MAX`）、
   `lru_head`（最旧）/ `lru_tail`（最新）；
   `LruNode { prev, next, table_idx, file_id, segment }`。
2. 表名反查用 `table_li: Vec<Option<TableIdent>>` + `free_table_li`，
   节点只存 `table_idx`，不按文件复制 `TableIdent`。
3. `FileState` / `SegmentState` 增加 `lru_idx`，删除 `last_used`；
   `CacheState` 删除 `clock`。
4. `lru_push` / `lru_unlink` / `lru_move_tail` 三个 O(1) 链表原语。
5. 状态变更接入：`upsert_resident` / `upsert_segment_resident`（entry API）、
   `mark_clean` / `mark_segment_clean`、`mark_dirty_range`、`touch` /
   `touch_segment`、`forget` / `forget_segment`、`evict_table`。
   dirty 文件一律不入链，flush 后入链尾，读命中挪链尾。
6. `evict_candidate` 改为读 `lru_head` O(1)，删除
   `oldest_clean_data` / `oldest_clean_segment` 两个全表扫描函数。

测试：
- 新增 `crate/engine/fkv_cloud/tests/cloud.rs::cache_budget_evicts_globally_oldest_across_tables`
  （两表交替、命中改序后淘汰全局最旧文件）。

验证：
- `cargo check -p fkv_cloud` 通过。
- `cargo nextest run -p fkv_cloud -E 'not test(concurrent_writers_never_roll_back_term)'`
  26 passed；既有 `concurrent_writers_never_roll_back_term` 在 HEAD 副本上
  3/20 复现失败（term 回退），与本次改动无关。
- `cargo clippy -p fkv_cloud --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

**通过。** 核实：`fkv_cloud/src/lib.rs:280-305`（`lru_head/lru_tail/node_li/free_node`、`LRU_NIL`、`pending_delete_li` 独立于本条）、:1106 `take_victim` O(1) 摘链；dirty 不入链的约束在 `upsert/mark_dirty_range` 各状态点（:508/:717/:789 等 `lru_unlink`）落实；全仓 grep 无 `oldest_clean_data/oldest_clean_segment`（已删）。测试 `tests/cloud.rs:716` `cache_budget_evicts_globally_oldest_across_tables` 在位。
