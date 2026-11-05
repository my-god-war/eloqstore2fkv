# sto-56a 页重写计划与空间放大口径对齐（L1 已实现）

## 位置

- `crate/core/fkv_mapping/src/lib.rs`：`MappingTbl::mapping_count`、`AppendAllocator::skip_to_next_file`、`PageMapper::{mapping_count, space_size, skip_empty_space, update_stat_by_scan}`
- `crate/engine/fkv_gc/src/lib.rs`：`compact_plan`
- `crate/engine/fkv_task_gc/src/lib.rs`：`live_page_li_by_file`
- `crate/engine/fkv_task_compact/src/lib.rs`：`compact_file_li`（批式搬页）
- `crate/engine/fkv_store/src/lib.rs`：`needs_maintain`、`compact_if_needed`、`compact_file_li`、`refresh_space_stat`、`install_replay_state`
- C++：`WriteTask::MapperExceedsAmplification` / `CompactIfNeeded`、`BackgroundWrite::DoCompactDataFile`

## 现状（已改）

1. 触发口径：`needs_maintain` 改用 `MappingCount()`（活跃映射数，O(1) 计数器）与
   `SpaceSize()`（扣除整文件空隙的已分配空间），不再用槽位总数 + high-water。
2. 重写计划：`fkv_gc::compact_plan` 一次产出全部超阈值文件（放大率降序），
   `Store::compact_file_li` 单批次、单 manifest 落盘完成多文件搬页。
3. 搬页：`compact_file_li` 只扫一遍 mapping 分组 live 页，按 `MAX_READ_PAGES_BATCH=128`
   批量读 + 批量写。
4. 空 mapping：`compact_if_needed` 在 `mapping_count()==0` 时把 high-water 推到下一文件边界
   并同步 `min_file_id`，`space_size` 归零，避免每轮写后维护重复触发。
5. 统计刷新：compact / gc / reopen 后调用 `update_stat_by_scan`，`space_size` 与盘面同步。

## 验收

- `cargo nextest run -p fkv_mapping -p fkv_task_compact -p fkv_store -p fkv_gc -p fkv_runtime --no-fail-fast` 全绿。
- `cargo clippy -p fkv_mapping -p fkv_task_gc -p fkv_task_compact -p fkv_store -p fkv_gc --no-deps --all-features --all-targets -- -D warnings` 零警告。
- 新增测试：`mapping_count_tracks_live_slots`、`skip_to_next_file_zeroes_space`、
  `compact_plan_covers_all_amplified_files`、`empty_mapping_stops_repeated_maintenance`。

## DONE（2026-11-04）

已实现并验证。segment 压缩与写任务 pending flag 见 sto-56b / sto-56c。

## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:144/:743`（`MappingTbl::mapping_count`）、`:400/:520`
（`space_size`）、`:417/:534`（`skip_to_next_file`）、`:893`（`update_stat_by_scan`）、
`:908`（`skip_empty_space`）；`fkv_gc/src/lib.rs:27`（`compact_plan` 一次产出全部超阈值
文件，放大率降序）；`fkv_task_gc/src/lib.rs:295`（`live_page_li_by_file` 单次扫描分组）；
`fkv_store/src/lib.rs:1787`（`install_replay_state` 后 `update_stat_by_scan`，注释引用
space_size 口径）。测试 `fkv_mapping/tests/mapping.rs:37/:71`、`fkv_gc/tests/gc.rs:13`、
`fkv_store/tests/gc_store.rs:151` 均存在。
