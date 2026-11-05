# perf-72 bench 只输出吞吐，无延迟分位，验收 p99 无法测量

## 位置
- Rust: `crate/perf/fkv_bench/src/main.rs:316-320`、`main.rs:334-345`、`main.rs:382-384`（指标收集）、`main.rs:22-46`（`PerfData`/`Run` 结构）
- C++: `benchmark/simple_bench.cpp:250-274`（write p50/p90/p99/p999/p9999）、`simple_bench.cpp:353-373`（read 分位）、`simple_bench.cpp:54-56`（`bvar::LatencyRecorder`）

## 现状
fkv_bench 端到端 workload 只收集吞吐指标：`put_kv_s`、`put_batch_s`、`get_kv_s`、`scan_kv_s`、`scan_page_s`（`main.rs:318-345`），全部是 `rows / elapsed` 或 `batch_num / elapsed`。
- `Run` 结构只有 `metric`/`value`/`unit`（`main.rs:36-46`），无延迟字段。
- 无任何 p50/p90/p99/p999 直方图或计数。

## C++ 实现 / 目标口径
C++ `simple_bench` 用 `bvar::LatencyRecorder`（`simple_bench.cpp:54-56`）按请求记录延迟，输出：
- write：average / p50 / p90 / p99 / p999 / p9999 / max（`simple_bench.cpp:250-262`）。
- read / scan：同样分位（`simple_bench.cpp:353-365`、`503-515`）。
- todo.md 验收标准第 6 条：**冷点读 p99 ≤ 1.2×**，明确要求 p99。

## 差异与风险
- fkv_bench 输出的 `get_kv_s`（吞吐）无法换算成 p99 延迟。验收项「冷点读 p99 ≤ 1.2×」**没有数据源**，CI 与看板都无法判定达标。
- 吞吐高不代表尾延迟低：fkv 若用单请求串行 await（见 perf-73），吞吐可能不低但 p99 极高，当前指标完全掩盖该风险。
- web 看板 `App.svelte` 只展示 value（吞吐）与 ratio，无延迟维度。

## 对齐建议
- fkv_bench 增加延迟直方图：每请求记录 `Instant::now()` 前后差值，收集 p50/p90/p99/p999/max。
- 指标命名对齐 C++：`get_p50_us`、`get_p99_us`、`get_p999_us`、`get_max_us`，写入 `Run`。
- read-only workload 下按 C++ 口径持续跑 `test_secs`（默认 60s），按时间窗口输出分位，而非固定 `rows*read_per_part` 次。
- 看板增加延迟 metric 维度。

## 性能优化建议
- 延迟测量用 `coarsetime::Instant` 或 `std::time::Instant`（单调），避免墙钟跳变；todo.md 选型指定热路径用 `coarsetime`。
- 分位统计用固定桶直方图（对齐 `fkv_metrics` 17 桶），O(1) 更新，避免排序。

## 严重度
P0（测量失真/验收无法达标）

## 审查结论（2026-11-04）

**判定：合理，需实现。**

核验结果：
- `main.rs` 端到端 workload 确实只有 5 个吞吐指标，`Run` 无延迟字段，无任何分位统计。属实。
- C++ `simple_bench.cpp` 不在本仓（`cpp/eloqstore` 为空目录），但 `doc/metrics.md` 与 `todo.md` §7.6 明确要求「冷点读 p99 ≤ 1.2×」，当前无数据源。属实。
- 分位桶口径：`fkv_metrics::BUCKET_UPPER_US` 为 17 桶（50µs 起点，上界 5s），粒度太粗，直接用于 bench p99 会把 1ms 延迟报成 2.5ms，误差不可接受。故 bench 侧用**细粒度自建直方图**（0..65536µs 每 µs 一桶 + 2 的幂溢出桶），只在 `bench` 内使用，不改 `fkv_metrics` 线上口径；跨语言对账时按 `doc/metrics.md` 说明先统一桶边界。
- 时钟：`coarsetime::Instant` 在 Linux 走 `CLOCK_MONOTONIC_COARSE`（分辨率 1–4ms），用于 µs 级分位会全部量化到 0 或 1000µs，**不可用**。故 per-request 延迟用 `std::time::Instant`（单调、µs 级），并在注释注明；吞吐窗口的 coarsetime 迁移归 perf-77。
- 「read-only 持续跑 test_secs」属于 workload/参数语义（perf-74）；「看板增加延迟维度」属于看板（perf-76）。本 issue 只负责 fkv_bench 产生延迟数据。

拆分为 1 个 todo（见 `issue/todo/perf-72a-bench-latency-percentiles.md`）。

## 复审（2026-11-05 r6）

通过（拆分记录，落地在 perf-72a）。

- 拆分决议（bench 自建细粒度直方图、不用 fkv_metrics 17 桶、per-request 用
  std Instant 不用 coarsetime 粗钟）与 perf-72a/77/77a 的落地链一致；
  72a r6 小节已核实 `LatencyHist` 与 `*_us` 指标在位。
