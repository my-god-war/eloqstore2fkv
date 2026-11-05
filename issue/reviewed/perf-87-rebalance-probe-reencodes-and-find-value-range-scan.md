# perf-87 P3：`rebalance_data_pages` 切分点试探每步全量重编码两页（最坏 O(n²)）；`find_value_range` 同族线性扫

来源：第 5 轮 store/write lane 侦查（子代理摘录；eng-53 遗留项的量化补充）。

## 证据

`crate/engine/fkv_task_write/src/lib.rs:1114-1131`：

```rust
let k0 = split_by_bytes(&entries, target, page_size, restart_interval)?;
for delta in 0..n {
  for k in [k0 + delta, k0.saturating_sub(delta)] {
    let Some(first) = build_raw_page(&entries[..k], page_size, restart_interval)? else { continue; };
    let Some(second) = build_raw_page(&entries[k..], page_size, restart_interval)? else { continue; };
```

每个候选切分点完整重编码前后两页；大 entry 居后时试探 O(n) 轮 × O(n) 编码。
C++ `Redistribute`（`batch_write_task.cpp:1118-1176`）用 region 迭代器到
`average_len` 后 region 直拷，单遍 O(page)。

`find_value_range`（`:923-931`）对每个 overflow slot 线性扫整页，同族。

## 目标

- `build_raw_page` 的 builder 暴露当前编码长度（前缀和/`current_size_estimate`），
  切分可行性用长度判断，试编码次数降到 1；
- 或照 fmt-17 的 region 直拷路径（已有 `PageRegionIter` / `FastIndexPageBuilder`
  基建，`fkv_page::region`）实现 `Redistribute` 单遍版。
- `find_value_range` 改为单遍收集（一次遍历同时定位各 slot 边界）。

## 验收

- 触发重分配的批（含不可切分 fallback）产物字节与现状一致（既有 fmt-17 等价
  测试兜底）；微基准或计数断言编码次数为常数。
- `cargo nextest run -p fkv_task_write`。

## 依赖 / 重叠

- eng-53（done）遗留性能项的量化立单；fmt-17/17b（done）region 直拷基建可复用。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上。

## DONE（2026-11-04）

- `fkv_page` 新增 `EntrySize` / `entry_size` / `page_content_len`：`DataPageBuilder::add` 与切分估算共用同一编码尺寸口径，无重复实现。
- `rebalance_data_pages`：`split_lens` 正向求前缀页长度、反向按 `i % restart_interval` 分组求后缀页长度，`pick_split` 直接用长度判定候选；每对页只实际编码 2 次（原最坏 O(n²)）。
- 选点顺序（`k0, k0+1, k0-1, ...`）、页面字节与分隔键均与旧逐点试编码一致；`pick_split_matches_bruteforce_probe`、`split_lens_matches_encoded_page_lengths` 兜底。
- `write_overflow_values`：slot 按 `(page_idx, key)` 排序，`locate_value_ranges` 每页单遍定位全部 value 区间（原每 slot 线性扫整页）。
- 验证：`cargo check` 三 crate 通过；`cargo nextest run -p fkv_task_write -p fkv_btree -p fkv_page -p fkv_store` 193 passed；clippy 三 crate `-D warnings` 零警告。

## 复审（2026-11-05 r6）

通过（注：实现在 `fkv_task_write`/`fkv_page`，按任务点名交叉核验，主体属写路径 lane）。
`fkv_task_write/src/lib.rs:1160`（`rebalance_data_pages`）、`:1252`（`pick_split`）、
`:1300`（`split_lens`）、`:989`（`locate_value_ranges` 每页单遍定位全部 value 区间）；
`fkv_page/src/data.rs:54/67/95`（`EntrySize` / `page_content_len` / `entry_size` 统一尺寸
口径）。测试 `fkv_task_write/src/lib.rs:2426`（`split_lens_matches_encoded_page_lengths`）、
`:2508`（`pick_split_matches_bruteforce_probe`）存在。
