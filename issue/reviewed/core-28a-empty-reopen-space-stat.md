# core-28a P3：空映射 reopen 的 `space_size` 改从尾文件起算（对齐 C++ `Replayer`）

源 issue：`core-28`（已并入本文件）。

## 位置

- `crate/core/fkv_mapping/src/lib.rs`：`PageMapper::update_stat_by_scan` 空分支。
- 消费端（不改）：`crate/engine/fkv_store/src/lib.rs` `Store::needs_maintain` /
  `compact_if_needed` / `install_replay_state`。

## 现状

```rust
let (Some(min_file), empty_file_cnt, _) = self.tbl.file_stat(shift) else {
  return; // min_file_id 维持 0
};
```

reopen 后 `mapped == 0 && space_size == max_fp_id != 0` →
首写多跑一轮全量 maintain；对齐边界时还会因 `skip_empty_space` 多烧一个文件。

C++ `replayer.cpp`（空 reopen）：`min_file_id = max_fp_id >> shift`，
`SpaceSize = max_fp_id % pages_per_file`（对齐时 0）。

## 方案

`update_stat_by_scan` 空分支改为：

```rust
let min_file = self.allocator.max_fp_id() >> pages_per_file_shift;
self.allocator.update_stat(min_file, 0);
```

- 不推进 `max_fp_id`（snap-up 仍由 `skip_empty_space` 在 `mapping_count == 0`
  且 `space != 0` 时执行，对齐边界不会多跳）。
- `Pooled` 分支 `update_stat` 仍是 no-op。
- 同步更新函数文档：空表也回写尾文件统计。

## 验收

- `fkv_mapping` 单测：`PageMapper::new(Append, 6)` + shift=2 →
  `min_file_id == 1`、`space_size == 2`；`max_fp_id = 8` → `space_size == 0`。
- `fkv_store` 集成测试：有历史写后清空（truncate）→ reopen →
  `space_size == max_fp_id % (1<<shift)`、`min_file_id == max_fp_id >> shift`；
  对齐情形 `needs_maintain` 为假。
- `cargo nextest run -p fkv_mapping -p fkv_store`、clippy 零警告。

## 审查结论（2026-11-04）

采纳，P3；详见源 issue 的审查结论。

## DONE（2026-11-04）

- `fkv_mapping`：`PageMapper::update_stat_by_scan` 空映射分支改为
  `min_file_id = max_fp_id >> shift`、`empty_file_cnt = 0`，并加单调保护。
- 测试：新增 `update_stat_by_scan_empty_uses_tail_file`（fkv_mapping）、
  `empty_reopen_space_stat_starts_from_tail_file`（fkv_store）。
- 验证：`cargo check -p fkv_store -p fkv_mapping` 通过；
  `cargo nextest run -p fkv_store -p fkv_mapping -p fkv_runtime` 173 通过 / 0 失败；
  `cargo clippy -p fkv_store -p fkv_mapping --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。同 core-28：`fkv_mapping/src/lib.rs:894-899` 空分支按尾文件起算，未推进 max_fp_id（snap-up 仍由 `skip_empty_space` :908-910 负责）；两个测试（mapping.rs:352 / gc_store.rs:190）在位。
