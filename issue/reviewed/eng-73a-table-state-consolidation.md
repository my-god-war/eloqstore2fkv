# eng-73a Store 每表状态收敛到单一 TableState；touch_table 默认零开销

## 位置

- `crate/engine/fkv_store/src/lib.rs`：`Store` 字段、`evict_table`、`reset_tables`、
  `open_empty`、`install_replay_state`、`put_at_inner`、`append_mapping_log`、
  `truncate`、`clean_expired`、`create_archive`、`create_branch`、`compact_file`、
  `put_large_impl`、`needs_maintain`、`compact_if_needed`、`gc`。

## 方案

1. 新增 `TableState`，字段直出：
   `mapper` / `segment_mapper` / `root`（`Some` 表示已打开）/ `ttl_root` /
   `branch_metadata` / `manifest_started` / `manifest_size` / `next_expire` /
   `compression` / `index_cache` / `ttl_pending`。
   `Store` 只保留 `table_map: HashMap<TableIdent, TableState>`。
2. `evict_table` / `reset_tables` / `drop_table` 变成删/清一个 map，消除三处手写清理。
3. `branch_metadata` 改 `refresh_branch_metadata(&mut self)` 原地更新
   `file_ranges.last_mut()`，`snapshot_record` 直接借出引用，删除两次 clone。
4. `touch_table`：`max_open_tables == 0` 直接返回；开启上限时用
   `table_order: VecDeque<(TableIdent, u64)>` + `touch_seq` 代际号，陈旧项惰性跳过，
   队列按需压实，触摸与驱逐均摊 O(1)，不再线性 `remove`。

## 验收

- 语义 / 生命周期等价：open、evict、truncate、drop、reopen、TTL、压缩、
  branch manifest、LRU 驱逐行为不变。
- 新增 `crate/engine/fkv_store/tests/table_state_cleanup.rs`：
  evict / drop 后 `is_open`、字典、index cache、`pending_ttl_len` 全部清零；
  默认配置不驱逐任何表。
- `cargo check -p fkv_store`
- `cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast`
- `cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings` 零警告

## DONE（2026-11-04）

- 新增 `TableState`（`crate/engine/fkv_store/src/lib.rs`），`Store` 只剩
  `table_map`；11 个平行 map + 3 处手写清理全部收敛为单条 remove / clear。
- `open_empty` / `install_replay_state` / `put_at_inner` / `truncate` /
  `clean_expired` / `create_archive` / `create_branch` / `compact_file` /
  `put_large_impl` / `gc` 改为字段写入。
- `branch_metadata` → `refresh_branch_metadata` 原地更新 `last_mut()`，
  删除两次整链 clone。
- `touch_table`：`max_open_tables == 0` 直接返回；开启上限时用
  `(表, 代际号)` 队列 + `touch_seq`，陈旧项惰性跳过、队列按需压实，均摊 O(1)。
- 新增 `crate/engine/fkv_store/tests/table_state_cleanup.rs`（3 个清理路径测试）。
- 验收：`cargo nextest run -p fkv_store -p fkv_runtime` 108 passed, 1 skipped；
  clippy `-D warnings` 零警告；`cargo fmt --check` 通过。

## 复审（2026-11-05 r6b）

通过。

- `TableState`（`crate/engine/fkv_store/src/lib.rs:154`）；`Store` 仅持 `table_map: HashMap<TableIdent, TableState>`（`:206`），`mapper_map`/`segment_mapper_map`/`root_map` 等 11 个平行容器名全文件 0 命中。
- `touch_table` 零开销短路：`:521-524`（`max_open_tables == 0` 直接返回）+ `(表, 代际号)` 队列惰性压实（`:526-540`），无 `VecDeque::remove` 线性路径。
- `evict_table` 单条 remove：`:473`。
- 测试：`crate/engine/fkv_store/tests/table_state_cleanup.rs:17 evict_table_clears_whole_table_state`、`:47 drop_table_clears_whole_table_state`、`:65 default_config_keeps_all_tables_open`。
