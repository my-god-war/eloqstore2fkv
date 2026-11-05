# perf-80 bench 不测 RSS，验收 RSS ≤ 1.1× 无数据源

## 位置
- Rust: `crate/perf/fkv_bench/src/main.rs:148-179`（run 收集，无 RSS 字段）、`main.rs:22-46`（`PerfData`/`Run` 结构无 RSS）、`.github/workflows/perf.yml:48-59`（bench 步骤无 RSS 采集）
- C++: `include/kv_options.h:224`（`GlobalRegisteredMemory` per-shard capacity）、`include/storage/root_meta.h:236`（`capacity_bytes_`）、`include/storage/page_mapper.h:72`（`capacity()`）

## 现状
- fkv_bench 的 `Run` 只有 `metric`/`value`/`unit`（`main.rs:36-46`），无任何内存/RSS 指标。
- perf.yml bench 步骤（`perf.yml:48-59`）不采集 `getrusage`/`/proc/self/statm`/`/proc/self/status VmHWM`。
- web 看板 `App.svelte` 无 RSS 维度。

## C++ 实现 / 目标口径
- C++ 通过 `GlobalRegisteredMemory` per-shard capacity（`kv_options.h:224`）、`RootMeta::capacity_bytes_`（`root_meta.h:236`）、`PageMapper::capacity()`（`page_mapper.h:72`）显式管理内存预算。
- todo.md 验收标准第 6 条：**RSS ≤ 1.1×**（同机对比 eloqstore）。
- C++ simple_bench 虽不直接打印 RSS，但内存预算可控可查。

## 差异与风险
- **RSS 验收无数据源**：fkv_bench 不测 RSS，验收项「RSS ≤ 1.1×」无法在 CI 或看板判定。
- Rust 侧索引缓存 `IndexCache`（`fkv_btree/src/lib.rs:133-229`）有 byte budget，但无全局 RSS 上限，内存超预算时无法被发现。
- 无 RSS 历史曲线，无法观察内存泄漏或增长趋势。

## 对齐建议
- fkv_bench 在每 run 前后采集 `getrusage(RUSAGE_SELF).ru_maxrss` 或 Linux `/proc/self/statm` resident 字段，写入 `Run { metric: "rss_kb", ... }`。
- 或独立 `rss_kb` metric，与吞吐/延迟并列。
- perf.yml bench 步骤增加 RSS 采集（`grep VmHWM /proc/self/status`）。
- 看板增加 RSS metric 维度与 ratio（rust/cpp ≤ 1.1）。

## 性能优化建议
- Rust 侧对齐 C++ 内存预算：`IndexCache` byte budget + 全局 `GlobalRegisteredMemory` 等价物（segment 池预算），超预算触发淘汰而非无界增长。
- 采集 RSS 用 `libc::getrusage`，避免解析 proc 文本。

## 严重度
P0（测量失真/验收无法达标）

## DONE（2026-11-04）

补齐「RSS 验收无数据源」这一条（`crate/perf/fkv_bench/src/main.rs`）：

- 新增 `peak_rss_kb()`：读 `/proc/self/status` 取 `VmHWM`（进程峰值 RSS，kB）；
  无 `/proc`（macOS）返回 `None`，不编造数字。解析逻辑拆成纯函数 `parse_vmhwm(&str)`
  便于测试。
- workload 跑完后追加一条 `Run { metric: "rss_peak_kb", unit: "kB" }`，
  与吞吐/延迟指标并列进入 `perf.json`，同时打印 `rss_peak_kb`。
- 单测：`tests::parse_vmhwm_reads_peak_rss_field`、
  `tests::parse_vmhwm_absent_or_malformed_is_none`。

验证：
- `cargo nextest run -p fkv_bench` 12 passed；
- `cargo clippy -p fkv_bench --all-targets --no-deps -- -D warnings` 零警告；
- 端到端 smoke：`cargo run -p fkv_bench -- --max-key 2000 --rows 500 --root ...
  --history ...` 正常产出 JSON（本机 macOS 无 `/proc`，故本轮没有 `rss_peak_kb` 条目；
  Linux/CI 上才会出现）。

消费侧不属本条：`perf.yml` 的 RSS 采集步骤归 `perf-75a`（同一文件，避免并发改），
看板 RSS 维度与 `rust/cpp ≤ 1.1` ratio 归 `perf-75b` / `perf-76`。
跨平台 RSS（macOS `getrusage`）未做：需引入 `libc`，本仓库当前只在
`fkv_io_compio` 用，暂不为此加依赖。

## 复审（2026-11-05 r6）

通过。

- `fkv_bench/src/main.rs:210 parse_vmhwm`（读 /proc/self/status VmHWM）、
  `:221 peak_rss_kb`（无 /proc 返回 None 不编造）；`:389-400` workload 后追加
  `Run { metric: "rss_peak_kb", unit: "kB" }` 并打印。
- 测试 `:1221 parse_vmhwm_reads_peak_rss_field`、
  `:1227 parse_vmhwm_absent_or_malformed_is_none`。
- gate.mjs 侧 rss/mem 前缀 max 1.1 策略已预留（见 perf-75b r6），链路闭合。
