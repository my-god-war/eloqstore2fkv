# perf-77a fkv_bench 时钟口径收敛：时间戳用 coarsetime，吞吐/延迟保留单调 Instant

## 位置
- `crate/perf/fkv_bench/src/main.rs:403` `now_secs()`（`SystemTime` + `UNIX_EPOCH` + `unwrap_or(0)`）。
- 同文件 `bench_workload` 各阶段窗口起点（`let started = Instant::now()`）与每请求 `req_start`。
- 文件头 `use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH}`。

## 现状
- 输出时间戳（JSON `date` / `generated_at`、临时目录名）走 `SystemTime::now()`，未按 todo.md §4.1「时间用 `coarsetime`」选型，且失败时静默落 0。
- 三类时钟用途（墙钟时间戳 / 吞吐窗口 / per-request 采样）在代码里没有任何说明，跨语言对比时容易误判口径。

## 方案
1. 时间戳：`now_secs()` 改为 `coarsetime::Clock::now_since_epoch().as_secs()`，语义与 C++ `utils::UnixTs`（`system_clock`）一致，秒级精度足够（JSON `date` / 目录名）。
2. 吞吐窗口：**保留 `std::time::Instant`**。理由写进注释：窗口分辨率不得低于其包住的延迟采样；`--rows 64` 等小规模跑法单阶段可短于 1ms，`coarsetime`（Linux `CLOCK_MONOTONIC_COARSE`，1–4ms）会给出 0 秒 → 吞吐 `inf`，并与同阶段 `max_us > 0` 自相矛盾。C++ `high_resolution_clock` 在 libstdc++ 是 `system_clock`（墙钟，可被 NTP 回拨），fkv 用单调时钟是改进，不是差异；跨实现对比按比值看，无需模仿墙钟。
3. per-request 延迟：**保留 `std::time::Instant`**（perf-72a 结论，µs 分位不能用 COARSE 时钟）。
4. 在文件头加模块级注释，把三类时钟用途、各自时钟源、与 C++ 的对照及「为何不用 coarsetime」一次写清；`bench_gxhash` 的 300ms 窗口同样用 `std::time::Instant`（粗钟会让 300ms 窗口量化误差达 1.3%）。

## 验收
- `cargo check -p fkv_bench --all-targets`、`cargo nextest run -p fkv_bench --no-fail-fast`、`cargo clippy -p fkv_bench --no-deps --all-features --all-targets -- -D warnings` 全绿。
- `now_secs()` 无 `SystemTime` / `UNIX_EPOCH` / `unwrap_or(0)`；输出 `date`/`generated_at` 仍为 Unix 秒。
- 代码内可直接读到「哪段用 coarsetime、哪段用 Instant、为什么」。

## DONE（2026-11-04）

改动文件：`crate/perf/fkv_bench/src/main.rs`、`crate/perf/fkv_bench/Cargo.toml`（`cargo add coarsetime`，`Cargo.lock` 随之更新）。

实现：
- `now_secs()` 改 `coarsetime::Clock::now_since_epoch().as_secs()`，去掉 `SystemTime` / `UNIX_EPOCH` / `unwrap_or(0)`；JSON `date` / `generated_at` 与临时目录名仍为 Unix 秒，语义对齐 C++ `utils::UnixTs`（`system_clock`）。
- 文件头新增模块级「时钟口径」注释：时间戳=coarsetime 墙钟；吞吐窗口与 per-request 采样=`std::time::Instant`（`CLOCK_MONOTONIC`）。写明**为何不用** `coarsetime::Instant`（Linux COARSE 1–4ms，µs 分位失真；小规模跑法 phase 可 <1ms → 粗钟 0 秒 → 吞吐 `inf`，且与 `max_us > 0` 自相矛盾）、以及 `high_resolution_clock` 在 libstdc++ 是 `system_clock`（墙钟），单调时钟是改进不是偏差。
- `bench_gxhash` 300ms 窗口、写阶段窗口、`req_start` 三处加就地注释说明时钟选择。
- 新增单测 `now_secs_is_unix_seconds`：断言单位是秒且与墙钟差值 ≤2s（防毫秒/秒口径错 1000 倍）。
- 顺带修掉本文件里**既有的**两处 clippy 报错（非本 issue 引入，但阻塞 `-D warnings` 验收）：`index % del_ratio != 0` → `!index.is_multiple_of(del_ratio)`；`bench_workload` 8 参 → 引入 `ReadPlan { partitions, read_per_part }` 归并读侧参数（7 参），调用点同步更新。
- `cargo fmt -p fkv_bench` 规范化本文件（含其它 agent 留下的未格式化长行）。

验收：
- `cargo check -p fkv_bench --all-targets`：通过。
- `cargo nextest run -p fkv_bench --no-fail-fast`：16 passed，0 failed。
- `cargo clippy -p fkv_bench --no-deps --all-features --all-targets -- -D warnings`：0 warning。
- `cargo fmt -p fkv_bench -- --check`：干净。
- 实跑 `cargo run -q -p fkv_bench -- --rows 64`：`put_kv_s: 23494.86`、`get_p99_us: 51.00 us`、`generated_at: 1793788654`（2026-11-04 Unix 秒，单位正确）。

遗留（不在本 issue 范围）：
- `bench_gxhash` 循环内每轮读一次时钟（~20–30ns/次，4096B 哈希约百 ns 级，占比可感）。要消除需 `coarsetime::Updater` 后台刷缓存 + `Instant::recent()`，或按 64/128 轮批量检查；收益与复杂度需单独评估，未在本 issue 动。
- C++ `simple_bench.cpp` 不在本仓（`benchmark/` 缺失），跨实现时钟对照只能按上游语义推断。
- `now_secs` 的秒级墙钟在极端时钟回拨下可能重复，目录名已用 `id()` + 序号兜底。

## 复审（2026-11-05 r6）

通过。

- `fkv_bench/src/main.rs:456-458 now_secs` =
  `coarsetime::Clock::now_since_epoch().as_secs()`，无 SystemTime/unwrap_or(0)；
  `Cargo.toml:23 coarsetime = "0.1.38"`。
- 文件头 `:8-20` 模块级「时钟口径」注释（三类用途、为何不用
  coarsetime::Instant、C++ high_resolution_clock 墙钟对照）。
- `:554 ReadPlan`（8 参归并）；测试 `:870 now_secs_is_unix_seconds`
  （秒级 + 与墙钟差 ≤2s）。
