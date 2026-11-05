# perf-77 计时口径 Instant vs C++ system_clock，且未用 coarsetime

## 位置
- Rust: `crate/perf/fkv_bench/src/main.rs:195`、`main.rs:299`、`main.rs:325`、`main.rs:338`、`main.rs:348`（全部 `Instant::now()`）、`main.rs:9`（`use std::time::{Duration, Instant, ...}`）
- C++: `benchmark/simple_bench.cpp:176`、`simple_bench.cpp:233`、`simple_bench.cpp:328`（`high_resolution_clock::now()`）、`simple_bench.cpp:171`、`simple_bench.cpp:314`（`utils::UnixTs<microseconds>()`）、`include/utils.h:40-46`（`UnixTs` 用 `system_clock`）

## 现状
- fkv_bench 全部用 `std::time::Instant`（单调时钟）计时（`main.rs:195/299/325/338/348`）。
- 吞吐口径：`rows / started.elapsed()`（`main.rs:317-318`）。
- 无 per-request 延迟测量，无 coarsetime 使用。

## C++ 实现 / 目标口径
- C++ 吞吐窗口用 `high_resolution_clock::now()`（`simple_bench.cpp:176/233/328`）。
- C++ 每请求延迟用 `utils::UnixTs<microseconds>()`（`simple_bench.cpp:171/314`），`UnixTs` 实现是 `system_clock`（`include/utils.h:40-46`，墙钟）。
- todo.md 选型：热路径用 `coarsetime`（单调时钟 / TTL），`jiff` 格式化。

## 差异与风险
- **时钟源不同**：fkv 用单调 `Instant`，C++ 延迟用墙钟 `system_clock`。墙钟可被 NTP/跳变影响，单调不会。跨实现对比时若一侧受墙钟跳变污染，p99 失真。
- **未用 coarsetime**：todo.md 指定热路径/计时用 `coarsetime`，fkv_bench 用 `std Instant` 违背选型，且 `Instant` 在部分平台分辨率低于 coarsetime 优化路径。
- **吞吐 vs 延迟时钟混用**：C++ 吞吐用 `high_resolution_clock`、延迟用 `UnixTs`（system_clock），fkv 全部 `Instant`，口径不统一，跨实现对比需注意。
- fkv_bench 无 per-request 延迟，故无墙钟跳变风险，但一旦按 perf-72 补延迟测量，需选对时钟。

## 对齐建议
- 吞吐窗口保持单调时钟（`Instant` 或 `coarsetime::Instant`），避免墙钟跳变。
- 若补 per-request 延迟（perf-72），用 `coarsetime::Instant` 单调时钟，对齐 todo.md 选型，并在文档注明与 C++ `system_clock` 的时钟源差异。
- 延迟单位对齐 C++：微秒（µs），非纳秒。

## 性能优化建议
- coarsetime 在热路径有更低开销与更好缓存局部性，bench 驱动大量请求时应切换。
- 吞吐与延迟统一用单调时钟，避免跨实现对比时时钟源混淆。

## 严重度
P2（性能优化）

## 审查结论（2026-11-04）

**判定：部分合理。** 采纳「时间戳口径对齐 / 时钟选择写清」，拒绝「per-request 与吞吐窗口改用 coarsetime」。

逐条核验：
- `main.rs:195/299/325/338/348` 的 `Instant::now()` 属实，但 perf-72/72a 已落地延迟直方图：`req_start`/`req_start.elapsed()` 是 **per-request µs 采样**。`coarsetime::Instant` 在 Linux 走 `CLOCK_MONOTONIC_COARSE`（`~/.cargo/registry/.../coarsetime-0.1.38/src/instant.rs`：`libc::CLOCK_MONOTONIC_COARSE`，分辨率 1–4ms），µs 分位会被量化成 0/1000µs，p99 验收直接失效。**必须保留 `std::time::Instant`。**
- 吞吐窗口（`started.elapsed()`）同样必须保留 `std::time::Instant`：`--rows 64` 等小规模跑法下单阶段可短于 1ms，粗粒度时钟会返回 0 → 吞吐除零为 `inf`，且与「同阶段 `max_us` > 0」自相矛盾（输出不可信）。吞吐窗口分辨率不得低于它所包住的延迟采样。
- 「`Instant` 分辨率低于 coarsetime 优化路径」不成立：`std::time::Instant` 在 Linux 是 `CLOCK_MONOTONIC` + vdso，精度高于 COARSE；coarsetime 的收益是「读缓存原子值零系统调用」，需要配 `Updater`，bench 每阶段只读 2 次，无收益。
- 「C++ 用 `system_clock` 墙钟、fkv 用单调时钟导致口径不一」：这是**改进**而非缺陷。墙钟会被 NTP 回拨，C++ 侧 p99 反而可能失真；跨实现对比时 Rust 侧单调时钟是更可信的一侧，只需在文档写明时钟源差异。
- 真实且未处理的部分：**输出时间戳**（`main.rs:403` `now_secs()` 走 `SystemTime` + `UNIX_EPOCH` + `unwrap_or(0)`）与 C++ `UnixTs`（`system_clock`）语义相同但未按 todo.md 选型用 `coarsetime`；三类时钟用途（墙钟时间戳 / 吞吐窗口 / per-request 采样）在全文件没有一处说明，跨语言对比时容易被误读。这部分拆为 `perf-77a`。
- 延迟单位已在 perf-72a 对齐为微秒（`*_us`），本条无需重复。

结论：拆 1 个 todo（`issue/todo/perf-77a-bench-clock-matrix.md`）。

## DONE（2026-11-04）

已按上节结论落地，子项见 `perf-77a-bench-clock-matrix.md`（同批移入 `issue/done/`）。

## 复审（2026-11-05 r6）

通过（本条为拆分记录，落地在 perf-77a）。

- 拆分决议（拒绝 per-request/吞吐窗口用 coarsetime 粗钟、保留单调 Instant、
  仅时间戳迁 coarsetime）经独立验证成立；`fkv_bench` 现状与 77a 落地一致
  （`now_secs` coarsetime + 窗口/采样 Instant，见 77a r6 小节）。
