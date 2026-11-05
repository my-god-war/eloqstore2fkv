# core-22a reopen term 不匹配时 allocator 跳到下一文件边界

## 位置
- `crate/engine/fkv_store/src/lib.rs`：`install_replay_state`、`branch_metadata`、新增 `segment_shift` / `next_file_boundary`。

## 方案
- 对齐 C++ `Replayer::GetMapper`：`self.term != 0 && state.branch_metadata.term != self.term` 时，data / segment 的 `max_fp_id` 均跳到 `((max >> shift) + 1) << shift`。
- data 用 `pages_per_file_shift`，segment 用 `segment_shift`（把 `branch_metadata` 里的硬编码 7 抽成方法复用）。
- 同 term reopen 行为不变。

## 验收
- 新 term reopen 后 `mapper.max_fp_id()` 落在文件边界，旧数据仍可读；同 term reopen 不 bump。
- 测试：`crate/engine/fkv_store/tests/term_bump.rs`。

## DONE（2026-11-04）
- 实现：`install_replay_state` 计算 `bump`，data / segment allocator 分别上跳边界；`next_file_boundary` 抽为自由函数；`segment_shift` 消除重复字面量。
- 测试：`crate/engine/fkv_store/tests/term_bump.rs`（同 term 不 bump、跨 term bump、旧数据可读、新写入不复用旧 file id）。
- 验证：`cargo nextest run -p fkv_store -p fkv_cloud` 96 passed；`cargo clippy -p fkv_store -p fkv_cloud -p fkv_io --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:1769-1775` bump 计算（data 用 pages_per_file_shift，segment 同理）、`:147-148` `next_file_boundary` 自由函数；`tests/term_bump.rs:34-48`（同 term 不 bump :34、跨 term bump 到 `((max>>shift)+1)<<shift` :42-43、新写不复用旧 file id :46-48）。
