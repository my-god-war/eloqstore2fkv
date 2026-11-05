# rev-03 bench scan 阶段未复刻 simple_bench 的并发模型与随机区间，且缺 `--scan-bytes`

## 位置

- Rust：`/Users/z/git/db/kv/fkv/crate/perf/fkv_bench/src/main.rs:872-895`
  （全量 scan：`for (tbl, store) in &store_li` 串行逐分区 `store.scan(tbl, b"", b"", 0)`，
  同一时刻仅 1 个 scan 在飞，窗口模式整轮重复）、
  `main.rs:897-951`（分页 scan：同样逐分区串行、从 `b""` 扫到表尾）
- C++：`/Users/z/git/db/kv/eloqstore/benchmark/simple_bench.cpp:394-545`
  （`num_scanners = FLAGS_read_per_part * FLAGS_partitions`，`simple_bench.cpp:419`；
  每 scanner 完成即重发，全程并发；`simple_bench.cpp:439-469` 每次请求随机
  `begin = rand % max_key`，`FLAGS_scan_bytes==0` 时 end 为空扫到表尾，否则
  窗口 `[start, start + scan_bytes/val_size)`；`simple_bench.cpp:400,405`
  `SetPagination(256, 0)`——每个请求即一页）

## 现状

Rust 的 scan 计量分两段：`scan`（整分区全范围、单请求扫完）与 `scan_page`
（每页 256 条、从表头翻到表尾），两段都是**逐分区串行、单请求在飞**，
`read_per_part` / `partitions` 对 scan 只决定表数量，不决定并发；
没有 `--scan-bytes` 之类的区间参数，起始 key 恒为 `b""`。

## C++ 实现

scan workload 与 read workload 对称：`read_per_part × partitions` 个 scanner
并发在飞，每个请求是**一次随机起点的分页读**（begin 随机、end 按 `scan_bytes`
截断或扫尾、每页 256 条），prefetch 由 `ScanRequest` 负责；`test_secs` 时间窗停止。
因此 C++ 的 `simple_bench_scan` p50/p99 是「256 条页请求」的延迟，吞吐是所有
scanner 合计。

## 差异与风险

1. **并发口径不同**：`read_per_part` 在 C++ 里同时驱动读与 scan 的并发
   （flag 说明 "concurrent read/scan requests per partition"），Rust 只作用于点读。
   同参数下 C++ scan 并发 = `read_per_part × partitions`，Rust 恒为 1，
   scan 吞吐无法跨实现对比，也测不出引擎 scan 侧读并发上限。
2. **延迟语义不同**：C++ 每个 scan 请求 = 一页（≤256 条、随机起点）；Rust `scan`
   阶段一个请求 = 整分区全量。同名指标 `scan_p99_us` 与 C++ p99 不是同一物理量
   （差约 `rows/256` 倍）。`scan_page` 段虽然页大小一致，但顺序全表遍历 ≠
   随机起点页请求，命中路径与 prefetch 形态不同。
3. **缺 `--scan-bytes`**：无法复刻「定字节窗口」的 scan 负载，短窗口 scan 的
   引擎读放大行为不可测。

## 对齐建议

- scan 阶段构造 `partitions × read_per_part` 条并发 scan 链（复用点读的
  `ReadLane`/`join_all` 模式），每链独立 RNG、随机 begin；
- 把按页请求（256 条/页、begin 由上一页 end 续）作为与 C++ `ScanRequest` 对齐的
  主计量口径，全量 `scan` 保留为附加指标并在输出名上区分（如 `fullscan_*`）；
- 新增 `--scan-bytes`，0 = 扫到表尾，与 C++ 同口径换算 `scan_kvs`。

严重度：P2

## 核验记录

- 基线：eloqstore `82b774c`。
- 逐行对读 `simple_bench.cpp:394-545`（Scanner/ScanLoop）与
  `fkv_bench/main.rs:872-951`；确认 Rust 串行单飞、`b""` 起点、无 scan_bytes；
  确认 C++ 并发 scanner 数、随机 begin、分页 256、`stop_` 时间窗。
- 附带核对（本项**无问题**）：延迟分位数集 p50/p90/p99/p99.9/p99.99 + avg + max
  与 C++ `LatencyRecorder` 一致（`main.rs:72-78,174-185`）；点读并发 =
  `read_per_part × partitions` 已对齐；RSS 经 `/proc/self/status` `VmHWM` 上报
  （`main.rs:226-239`，C++ 侧无此项，属超出对齐范围的增项）。
- 区分：属「实现但不正确（负载形状）+ 部分未实现（`--scan-bytes`）」；
  计量链路本身（直方图/窗口/合并）正确。

## 甄别（2026-11-05）

判定：**采纳**，P2 成立。三项指控全部独立复核属实：

1. Rust 全量 scan（`main.rs:880-890`）：`for (tbl, store) in &store_li`
   串行逐分区 `store.scan(tbl, b"", b"", 0)`，begin 恒为 `b""`、单请求在飞、
   窗口模式整轮重复（`assert_eq!(scanned, live_num * scan_round)`）。
   分页 scan（`:903-943`）同样逐分区串行，`begin = Vec::new()` 从表头顺序
   翻到表尾，256 条/页但起点非随机。
2. `grep scan_bytes main.rs` 零命中；`BenchArgs`（`:594-602`）无任何 scan
   区间参数，指控 3（缺 `--scan-bytes`）成立。
3. C++ `simple_bench.cpp:419` `num_scanners = FLAGS_read_per_part *
   FLAGS_partitions`，`:476-479` 全量首发、`:483-488` 完成即重发，
   `:527 stop_` 时间窗；`:399 id % FLAGS_partitions` 轮转绑分区；
   `:444` begin = `rand_gen() % FLAGS_max_key`；`FLAGS_scan_bytes==0`
   end 为空扫尾，否则 `scan_kvs = max(1, scan_bytes/val_size)` 窗口；
   `:400,405 SetPagination(256, 0)`；`:30` flag 说明
   "concurrent read/scan requests per partition" 证实 `read_per_part`
   在 C++ 同时驱动 read 与 scan 并发。与 issue 引述逐条一致。

去重边界（全目录 grep `scan_bytes`/scan 相关 issue）：

- **perf-73（reviewed，已落地）**：只修读/写阶段的串行单飞——其 C++ 引证
  限于 `:181-197`（Writer）与 `:304-335`（ReadLoop），ScanLoop `:394-545`
  从未入其范围；DONE 产出 `ReadLane`/`join_all` 仅覆盖点读，当前
  `main.rs:872-951` scan 仍串行即为其未覆盖面。本 issue 是 perf-73 在
  scan 阶段的补集，非重复。
- **perf-71（reviewed）**：点读热顺序/冷随机与 key 编码；其甄别明确把
  `read_per_part` 并发语义拆给 perf-73，scan 指标仅按原样保留，未触及
  scan 负载形状。
- **perf-84（reviewed）**：引擎侧 `Store::scan*` 整范围物化（max_entries
  不下推），属引擎实现缺陷，非 bench 负载形状；正交。
- **perf-95（reviewed）**：只补 `test_secs` 时间窗；scan「整轮重复」语义
  正是串行单飞的产物，本 issue 落地并发后需按链内入窗重述（见下）。
- **rev-01（todo，kv_size 语义）/ rev-02（new，写负载模型）**：分别属
  写侧参数语义与写负载形状，与 scan 阶段零交集（rev-02 全文无 scan）。

开发计划：

1. **并发 scan 链**：复用点读 `ReadLane` 模式构造
   `partitions × read_per_part` 条链，每链绑定分区（`id % partitions` 轮转，
   对齐 C++ `:399`）、独立 RNG seed（`lane_seed` 派生）、独立 `LatencyHist`
   后 merge；时间窗改为链内 gate（对齐 perf-95 `run_read_lane` 口径），
   废除「整轮重复 + scanned == live_num × round」断言，改按实际完成数计量。
2. **随机区间**：每请求 `begin = key_bytes(rng.usize(0..rows))`；
   `--scan-bytes 0` 时 end 空、扫到表尾（对齐 `:444-449`）；非 0 时
   `scan_kvs = max(1, scan_bytes / value_size)`、
   `start ∈ [0, rows - scan_kvs)`、`end = key_bytes(start + scan_kvs)`
   （对齐 `:452-465`）。每请求一页 256 条（`SetPagination(256, 0)` 语义），
   以此为主计量口径 `scan_*`，对齐 C++ `ScanRequest` 的 p50/p99 物理量。
3. **指标命名拆分**：现有全量 scan 改名 `fullscan_kv_s`/`fullscan_*`
   保留为附加指标（顺序全表遍历另有诊断价值，perf-71 甄别已有此意）；
   `scan_page_*` 顺序翻页段可并入 fullscan 附注或删除，避免同名异义。
4. **参数**：`--scan-bytes`（u64，默认 0 = 扫尾）进 `BenchArgs` 与
   `PerfData` JSON（可复现），帮助文本对齐 C++ `:45`。
5. **测试**：链数 = `partitions × read_per_part` 断言（仿
   `read_lanes_match_cpp_reader_num`）；scan_kvs 换算边界（0、> max_key、
   非 value_size 整倍数）；随机窗口 end 截断正确性；窗口模式多链直方图
   merge 后计数一致；`--scan-bytes` 序列化进 JSON。

## DONE（2026-11-05）

**状态：已完成，issue 关闭。**

改动（`crate/perf/fkv_bench/src/main.rs`）：

1. scan 主阶段改为 `partitions × read_per_part` 条并发链（`ScanLane`，
   对齐 C++ `num_scanners`），分区按 `id % partitions` 轮转，独立 RNG /
   独立直方图，阶段末合并；每链完成一页即发下一请求。
2. 每请求一页 256 条（`PageOption { page_entries: 256 }`，对齐
   `SetPagination(256, 0)`）、随机起点；`begin = key_bytes(start × key_interval)`。
3. 新增 `--scan-bytes N`（缺省 0）：0＝end 空、扫到表尾；否则
   `scan_kvs = max(1, N / value_size)`，`start ∈ [0, rows - scan_kvs)`、
   `end = start + scan_kvs`（与 C++ `send_req` 同公式）。`PerfData` 增
   `scan_bytes`（`#[serde(default)]`）。
4. 指标口径拆分：`scan_kv_s` / `scan_page_s` / `scan_*_us` 改为页请求口径
   （C++ `ScanRequest` 物理量）；旧全表遍历改名 `fullscan_kv_s` /
   `fullscan_*_us` 保留为附加指标；旧 `scan_page_*` 延迟段删除。
5. 时间窗改为链内 gate（对齐 `run_read_lane`）：窗口内逐请求循环，固定次数
   模式每链 1 个页请求；废除「整轮重复 + scanned == live_num × round」
   断言，fullscan 改为逐轮条数恒等断言。

测试：

- 新增 `scan_lanes_match_cpp_scanner_num`（链数 / 分区轮转 / 独立种子）、
  `scan_kvs_matches_cpp_conversion`（0、非整倍数、>rows 边界）、
  `scan_range_truncates_window`（end 截断 / 扫尾）、
  `churn_and_scan_bytes_smoke`（覆盖率写 + 窗口 scan）、
  `perf_data_new_fields_back_compat`（旧 JSON 缺字段、回读）。
- 既有 workload 指标测试全部按新口径更新；`cargo test -p fkv_bench` 34 passed；
  `cargo clippy -p fkv_bench --all-targets` 无警告；`cargo +nightly fmt -p fkv_bench`。
- 冒烟：`--partitions 4 --read-per-part 2 --scan-bytes 320 --write-batchs 32
  --test-secs 1` 通过，`scan_page_s > 0`、`scan_kv_s` 与页数/条数一致。

风险 / 有意偏离：

- 分片语义下随机起点可能落在本分区最后一个 key 之后，返回空页；这是
  「分区=同键空间分片」的真实反映，`partitions=1` 时与 C++ 完全同构。
- `scan_kv_s` / `scan_page_s` / `scan_*_us` 语义断代：旧 `scan_kv_s` 是全表
  遍历吞吐、旧 `scan_page_*` 是顺序翻页；跨代 `perf.json` 不可直接连曲线，
  已在 doc/metrics.md §6.2 登记。
- 固定次数模式（`--test-secs 0`）每链只发 1 个页请求；C++ 无对应口径，
  稳态比较请用默认 60s 窗口。

## 复审（2026-11-05 r7）

**判定：通过。** 逐条按 DONE 核对源码与测试（`fkv_bench/src/main.rs` 行号为
复审时点读数；C++ 侧亲读对照）：

- **并发链属实**：`ScanLane` + `scan_lane_li` 链数
  `partitions × read_per_part`、`id % partitions` 轮转、`scan_lane_seed` 按链
  编号派生（`main.rs:1021-1046`）；`scan_lanes_match_cpp_scanner_num` 断言
  `4×3 → 12` 链、分区 2 恰 3 链、12 个种子互异、空表也建满 scanner 数
  （`:2003-2013`）。驱动 `join_all` 并发（`:1369-1373`），与 C++
  `num_scanners = read_per_part × partitions`（`simple_bench.cpp:419`）对齐。
- **随机区间 + 窗口公式逐字对齐 C++**：亲读 `simple_bench.cpp:443-464`
  `send_req`——`scan_bytes==0` 时 `begin = rand % max_key`、end 空扫尾；否则
  `scan_kvs = max(1, bytes / val_size)`、
  `max_start = max_key > scan_kvs ? max_key - scan_kvs : 0`、
  `start = max_start > 0 ? rand % max_start : 0`、`end = start + scan_kvs`。
  Rust `scan_range`（`:1062-1073`）与 `scan_kvs`（`:1052-1054`）同公式、
  同边界（含取不到 `max_start` 本身），`value_size = kv_size - 8` 口径两侧
  一致；`scan_range_truncates_window` / `scan_kvs_matches_cpp_conversion`
  覆盖 0 / 非整倍数 / 边界折算。
- **每请求一页 256 属实**：`SCAN_PAGE_ENTRIES = 256`（`:109`）+
  `PageOption { page_entries: 256 }`（`:1140-1143`），对齐
  `SetPagination(256, 0)`；时间窗改链内 gate（`:1173-1179`），固定次数每链
  1 个页请求，与风险段自述一致。
- **指标口径拆分属实**：`scan_*` 页请求主指标（`:1383-1389`）、旧全表遍历
  改名 `fullscan_*` 附加指标 + 逐轮条数恒等断言（`:1391-1414`，取代旧
  `scanned == live_num × round`）；断代登记在 `doc/metrics.md` §6.2
  （`:189-193`）。
- **正确性护栏**：debug 构建逐 key 校验属本分区、落请求区间、按
  `live_after` 存活（`:1153-1171`）——分片语义下随机起点落本分区末 key 之后
  返回空页是真实反映（与风险段一致）。
- `churn_and_scan_bytes_smoke` 覆盖写覆盖 + 窗口 scan 端到端（`:1713-1748`）；
  `perf_data_new_fields_back_compat` 覆盖 `scan_bytes` 序列化回读。

验证声称（cargo test 34 passed / 冒烟）因并行会话占锁未复跑，以代码 + 测试
文本核对为准。
