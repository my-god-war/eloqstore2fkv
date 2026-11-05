
## 复审（2026-11-05 r6）

通过。`fkv_gc/src/lib.rs:74-88`（`segment_compact_plan`：复用 `compact_plan_where`，
按 segment 槽位算放大率，跳过 `tail_file_id` 并注释为何不能用候选最大 id 代替）；
测试 `fkv_store/tests/segment_compact.rs`（2 个测试）存在；`Store` 侧
`segment_file_amplify_factor` 接线在 `fkv_store`（grep 确认符号存在，未逐行）。
DONE 遗留（fkv_shard/fkv_runtime 未透传）按当时的并行冲突记录在案。
