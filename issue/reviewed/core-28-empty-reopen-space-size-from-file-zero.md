# core-28 P3：空映射 reopen 后 `space_size` 从 file 0 起算，`needs_maintain` 每轮空转全量 GC

来源：第 5 轮 core lane 侦查（子代理摘录，与 core-26 同族不同点）。

## 证据

`crate/core/fkv_mapping/src/lib.rs:850-853`：

```rust
pub fn update_stat_by_scan(&mut self, pages_per_file_shift: u8) {
  let (Some(min_file), empty_file_cnt, _) = self.tbl.file_stat(pages_per_file_shift) else {
    return;   // 空映射：min_file_id 维持 0 → space_size = max_fp_id 全量历史
  };
```

C++（`replayer.cpp:364-368`）：空 reopen 时
`min_file_id = max_fp_id_ >> pages_per_file_shift`，allocator 从尾文件起步，
`SpaceSize = max_fp_id mod pages_per_file`（对齐时 0）。

## 问题

Rust 空表 reopen 后 `mapped==0 && space!=0` 恒成立 → `needs_maintain`
（`fkv_store:2738-2748`）每轮触发全量 maintain（列目录 + retained 扫描 + 删文件
IO），C++ 在对齐情形完全 no-op。叠加 core-26（多跳一个文件）时跳得更多。
收敛后两侧 space 都归 0，故 P3。

## 目标

`update_stat_by_scan` 空表分支对齐 C++：`min_file_id = max_fp_id >> shift`，
`space_size` 只算尾文件残量。与 core-26 一起修（同文件同族测试）。

## 验收

- 测试：有历史写后全删/清空 → reopen → `needs_maintain` 为假
  （对齐情形）；`space_size` 等于尾文件残量。
- `cargo nextest run -p fkv_mapping -p fkv_store`。

## 依赖 / 重叠

- sto-56a（非空扫描口径，done）正确；本条是空表分支。

## 核验记录

- 子代理摘录（未逐行复核）：Rust/C++ 行号如上；开发时验证。

## 审查结论（2026-11-04）

**采纳（部分事实修正）**。

核验（当前源码）：

- `crate/core/fkv_mapping/src/lib.rs` `update_stat_by_scan`：空分支 `return` 属实，
  `min_file_id` 维持 `AppendAllocator::new` 的 0，`space_size = max_fp_id`。
- `crate/engine/fkv_store/src/lib.rs` `needs_maintain`：`mapped == 0` 时返回
  `space != 0`，确实为真；`install_replay_state` 在 reopen 时调用
  `update_stat_by_scan`，空表分支直接返回，故 reopen 后首判为真。
- 修正：并非「每轮空转」。首次 `maintain` → `compact_if_needed` 空表分支
  `skip_empty_space` 把 `max_fp_id` 推到下一文件边界，`space_size` 归零，
  之后 `needs_maintain` 恒假。实际代价是 **reopen 后首写多跑一轮全量 maintain**
  （列目录 + retained 扫描 + GC），并对齐边界时多烧一个文件（core-26）。
  C++ 空 reopen 直接 `min_file_id = max_fp_id >> shift`、`space_size = max_fp_id % pages_per_file`，
  对齐时完全 no-op。收敛后两边都归零，故仍为 P3。

落地方案：`update_stat_by_scan` 空分支对齐 C++，`min_file_id = max_fp_id >> shift`、
`empty_file_cnt = 0`（`space_size` 自然等于尾文件残量）。拆分见
`issue/todo/core-28a-empty-reopen-space-stat.md`。

## DONE（2026-11-04）

- `fkv_mapping`：`PageMapper::update_stat_by_scan` 空映射分支改为
  `min_file_id = max_fp_id >> shift`、`empty_file_cnt = 0`，并加单调保护。
- 测试：新增 `update_stat_by_scan_empty_uses_tail_file`（fkv_mapping）、
  `empty_reopen_space_stat_starts_from_tail_file`（fkv_store）。
- 验证：`cargo check -p fkv_store -p fkv_mapping` 通过；
  `cargo nextest run -p fkv_store -p fkv_mapping -p fkv_runtime` 173 通过 / 0 失败；
  `cargo clippy -p fkv_store -p fkv_mapping --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:894-899` 空映射分支 `min_file = max_fp_id >> shift`、`empty_file_cnt = 0`（带 `tail_file >= min_file_id` 单调保护）；测试 `tests/mapping.rs:352 update_stat_by_scan_empty_uses_tail_file`、`fkv_store/tests/gc_store.rs:190 empty_reopen_space_stat_starts_from_tail_file`。
