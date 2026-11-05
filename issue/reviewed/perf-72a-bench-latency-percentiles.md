# perf-72a fkv_bench 增加每请求延迟直方图与 p50/p90/p99/p999/max

## 位置
- `crate/perf/fkv_bench/src/main.rs`：`bench_workload` 写/读/scan/scan_page 各阶段；`Run` 指标列表。

## 现状
- 仅 `put_kv_s` / `put_batch_s` / `get_kv_s` / `scan_kv_s` / `scan_page_s` 吞吐指标。
- 无延迟字段，验收 `p99 ≤ 1.2×` 无数据源。

## 方案
1. 新增 `LatencyHist`：`[0, 65536)` 微秒每 µs 一桶（`Box<[u32]>`，一次性分配），其上 32 个 2 的幂溢出桶；`record` O(1)，无每请求分配；`count`/`sum_us`/`max_us` 就地累计。
2. 每请求 `std::time::Instant` 前后计时（单调、µs 级；`coarsetime` 在 Linux 是 COARSE 时钟，分辨率 1–4ms，不适用于 µs 分位，注释注明）。亚微秒按 1µs 计，避免 0 值。
3. 阶段指标：`put` / `get` / `scan` / `scan_page` 各输出 `avg_us`、`p50_us`、`p90_us`、`p99_us`、`p999_us`、`p9999_us`、`max_us`，单位 `us`，命名对齐 C++ `LatencyRecorder` 的 average/p50/p90/p99/p999/p9999/max。
4. `bench_workload` 返回 `Vec<(String, f64, &'static str)>`，JSON 字段沿用 `Run{metric,value,unit}`。
5. 分位定义：目标排名 `ceil(count*percent/100)`，线性区返回 `index+1` µs（桶上界），溢出区返回桶上界。

## 验收
- `cargo check -p fkv_bench`、`cargo nextest run -p fkv_bench`、`cargo clippy -p fkv_bench --no-deps --all-features --all-targets -- -D warnings` 全绿。
- 单测覆盖：空直方图、线性区/溢出区分位、分位单调性。
- JSON 输出含 `get_p99_us` 等字段，单位 `us`。

## DONE（2026-11-04）

改动文件：`crate/perf/fkv_bench/src/main.rs`（仅此一个）。

实现：
- `LatencyHist`：`[0, 65536)` µs 每 µs 一桶（`Box<[u32]>`，一次分配）+ 32 个 2 的幂溢出桶；`record` O(1)、无每请求分配；`count/sum_us/max_us` 就地累计。
- 每请求 `std::time::Instant` 前后计时。**偏离「coarsetime」选型并已在代码注释与审查结论说明**：`coarsetime::Instant` 在 Linux 走 `CLOCK_MONOTONIC_COARSE`（分辨率 1–4ms），µs 分位会退化为 0/1000µs，不可用于 p99 验收；吞吐窗口的 coarsetime 迁移归 perf-77。
- 阶段指标：`put` / `get` / `scan` / `scan_page` 各输出 `avg_us`、`p50_us`、`p90_us`、`p99_us`、`p999_us`、`p9999_us`、`max_us`，单位 `us`。C++ 口径为 average/p50/p90/p99/p999/p9999/max（`bvar::LatencyRecorder`），字段名以 `_us` 后缀标识微秒，JSON 仍用 `Run{metric,value,unit}`。
- 分位：目标排名 `ceil(count*percent/100)` 并钳制到样本数；线性区返回真实 µs（误差 <1µs），溢出区返回桶上界并钳制到 `max_us`，保证 p99 ≤ max。
- 亚微秒按 1µs 计，避免分位出现 0。

验收：
- `cargo check -p fkv_bench --all-targets` 通过。
- `cargo nextest run -p fkv_bench --no-fail-fast`：10 passed。
- `cargo clippy -p fkv_bench --no-deps --all-features --all-targets -- -D warnings`：0 warning。
- 实跑 `fkv_bench --rows 64` 输出 `get_p50_us/get_p99_us/get_max_us` 等字段，p50 ≤ p99 ≤ p999 ≤ p9999 ≤ max。

遗留（不在本 issue 范围）：
- read-only 持续 `test_secs` 口径 → perf-74。
- 看板延迟维度 → perf-76。
- 吞吐窗口 coarsetime 迁移 → perf-77。
- 跨语言 p99 对比前需按 `doc/metrics.md` 统一桶边界。

## 复审（2026-11-05 r6）

通过。

- `fkv_bench/src/main.rs:75 LatencyHist`（65536 µs 线性桶 + 2 的幂溢出桶，
  `record` O(1)）、`:120 merge`、`:138` 分位排名向上取整并钳制样本数。
- 四阶段直方图：put `:704`、get `:770`、scan `:782`、scan_page `:796`；
  指标名 `:880`（avg_us/p50_us/p90_us/p99_us/p999_us/p9999_us/max_us）。
- 每请求 `std::time::Instant` 计时与「不用 coarsetime 粗钟」的理由注释
  （与 perf-77/77a 决议一致）。
