# perf-95 P2 bench 读/写/scan 各阶段按固定次数计量，无 `test_secs` 时间窗口，延迟分位与 C++ 稳态口径不可比

## 来源

r6 复审（2026-11-05，io/compio/bench lane）：复核 `perf-72`（延迟分位）与
`perf-74`（workload 参数语义）时发现归属链断裂——perf-72 把「read-only 持续
`test_secs`」明确推给 perf-74，perf-74 的审查结论与 DONE 均未接该项，
全 issue 目录 grep `test_secs` 无任何条目实际覆盖。属真空缺口，非重复立项。

## 证据

- Rust `crate/perf/fkv_bench/src/main.rs`（开发前快照 1237 行）全文无
  `test_secs` / `--test-secs` 参数；唯一时间窗是 `bench_gxhash` 的 300ms
  （`:421 while start.elapsed() < Duration::from_millis(300)`）。
- 读阶段按固定遍数：每链读该分区全部 key，总次数 = `rows * read_per_part`
  （`ReadLane`/`read_lane_li`，`:603/623`）；写/scan 同为次数驱动
  （`bench_workload` 各阶段 `started.elapsed()` 只作分母，开发前快照
  `:745/778/794/839`，原引 `:741/774/790/833` 漂移 4 行，已按 2026-11-05
  复核更正）。
- 归属链：`issue/reviewed/perf-72-bench-no-latency-percentiles.md`（原文
  「read-only workload 下按 C++ 口径持续跑 test_secs（默认 60s），按时间窗口
  输出分位，而非固定 rows*read_per_part 次」；审查结论「属于 workload/参数
  语义（perf-74）」）→ `issue/reviewed/perf-74-workload-param-semantics-mismatch.md`
  只落地了 kv_size 语义/key 编码/delete/稀疏 key/workload 枚举/参数进 history，
  无 `test_secs` 内容。

## C++ 对照

- `eloqstore/benchmark/simple_bench.cpp:31`：
  `DEFINE_uint32(test_secs, 60, "read/scan test time")`。
- `ReadLoop`（`:301-342`）：`num_readers` 个 reader 全部在飞，`while (true)`
  从完成队列收割、立即回填新请求（`:336-342`），持续向 `g_read_latency`
  （bvar LatencyRecorder）累计样本；主线程 `:627/:642` sleep
  `FLAGS_test_secs` 后停机（scan 分支同型）。
- 即 C++ 的 p50/p99/p999 是 **60 秒稳态窗口** 的分位；窗口足够长，冷启动、
  页缓存预热、首轮 compaction 等瞬态被稀释。

## 差异与风险

- **尾延迟口径失真**：Rust 侧窗口长度 = 固定次数 × 单请求延迟。`--rows 20000`
  默认下读阶段仅数秒；小规模跑法（CI 用 `--rows 20000`，本地常更小）窗口可短至
  毫秒级，p999/p9999 样本不足，冷启动噪声主导分位。gate 的 `p99 ≤ 1.2×`
  （`web/scripts/gate.mjs` 延迟类策略）两边窗口口径不同，比值不可比。
- **吞吐同理**：C++ 读吞吐 = 窗口内完成数 / test_secs（稳态）；Rust = 固定
  次数 / 总耗时（含预热段），系统性偏低。
- 现有 `read_per_part` 语义是「并发链数」（perf-73 已对齐 C++ 并发读者），
  再无参数可表达「跑多久」。

## 建议修法

1. `fkv_bench` 新增 `--test-secs <N>`（默认 60，对齐 C++ `test_secs`）；
   读阶段改为：并发链满在飞，单链完成一批即取下一批随机 key，直到
   `started.elapsed() >= test_secs`；分位与吞吐按窗口计算。
   次数上限保留（`rows * read_per_part` 遍数）作为 0 值 / 兼容回退
   （`--test-secs 0` = 现行为），保证既有 CI 参数不变。
2. scan / write 阶段同口径评估：C++ `test_secs` 覆盖 read/scan；write 按
   rows 计（C++ write 也是次数），保持次数驱动即可，无需扩。
3. `PerfData` 增 `test_secs` 字段（`#[serde(default)]`），history 记录口径。
4. `doc/metrics.md` / `fixtures/README.md` 若引用 bench 窗口语义，同步注明。

## 验收

- `--test-secs 2 --rows 2000` 实跑：读阶段实际耗时 ≈2s（±10%），JSON 含
  `test_secs=2`；`--test-secs 0` 行为与现状一致（固定遍数）。
- 窗口内 `LatencyHist` 样本数 ≥ 窗口内完成请求数（链回填无丢失）。
- `cargo nextest run -p fkv_bench`；clippy `-D warnings` 零警告。

## 依赖 / 重叠

- perf-72/72a（延迟直方图）、perf-73（并发链）、perf-74（参数语义）均已
  done/reviewed，本条只补时间窗口维度，无代码冲突。
- gate.mjs（perf-75b）无需改：分位字段名不变。

## 甄别（2026-11-05 r6）

**结论：采纳（P2 维持）。** 全部证据亲核属实，属真空缺口，且为
`issue/reviewed/00-r6-io-bench.md` 任务 B 的既定唯一新立项。

### 证据核实

- Rust `crate/perf/fkv_bench/src/main.rs`：CLI 全参数表（:226-259）无
  `--test-secs`；全仓 grep `test_secs` 于 rs/toml/mjs/yml 零命中（KvOptions
  亦无——C++ 侧 `test_secs` 是 bench gflag 而非 KvOptions，config 来源仅为
  bench CLI 参数）。唯一时间窗为 `bench_gxhash` 300ms（:421）。读链固定次数：
  `ReadLane`（:603）、`read_lane_li`（:623）、`run_read_lane` 的
  `for _ in 0..lane.count`（:655），总次数 `rows*read_per_part`（开发前快照
  :767，原引 :763 已更正）；写 `while start < rows`（:713，原引 :709）、
  scan/scan_page 亦次数驱动（:789-793/:804-838，原引 :785-789/:800-831），
  `started.elapsed()` 只作吞吐分母（:745/778/794/839，原引
  :741/774/790/833）。行号按开发前 1237 行快照更正，符号引用全部属实。
- 意见未提但加强其结论的事实：main.rs :755-760（原引 :752-755）写后显式
  reopen 丢弃 index
  cache 让点读走冷路径（perf-71 落地），即 Rust 分位是「一遍冷读」口径；
  C++ 是 60s 连续回填稳态口径，p99 比值系统性偏离，gate
  （`web/scripts/gate.mjs:12` `LATENCY_LIMIT=1.2`，:53-54 按 `p\d+`/`_p\d+`
  前缀识别延迟类、max 方向）直接消费该比值。
- C++ `eloqstore/benchmark/simple_bench.cpp:31`
  `DEFINE_uint32(test_secs, 60, "read/scan test time")` 属实；`ReadLoop`
  :301-380：`num_readers = read_per_part*partitions`（:304）满在飞
  （:332-335）、`while(true)` 收割即回填（:336-343）、持续累计进
  `g_read_latency`（bvar LatencyRecorder，:340）；主线程 `:627`/`:642` sleep
  `FLAGS_test_secs` 后置 stop_。write 按 `write_batchs` 固定批次（:186）——
  意见修法第 2 条「write 保持次数驱动」与 C++ 相符。
- 补充细节（实现时定夺）：C++ `test_secs` 只 gate 纯 `read`/`scan` workload；
  `write-read`/`write-scan` 混合模式下读/scan 循环跑到 WriteLoop 结束为止
  （:596-611）。Rust 加 `--test-secs` 时混合模式需决定读窗口语义
  （仅 read-only/scan 生效，或 `min(test_secs, 写阶段结束)`）。
- CI 口径：`perf.yml:35` `PERF_ROWS` 默认 20000，意见「CI 用 --rows 20000」
  属实；读阶段 8 万次点读窗口仅秒级且含 reopen 后冷启动段。

### 与相邻条目边界（去重）

- perf-72:26 原文与意见引用逐字一致；perf-72:45、perf-72a:40 均把 test_secs
  明确推给 perf-74；perf-74（reviewed）落地范围仅 kv_size 语义/key 编码/
  delete/稀疏 key/workload 枚举/参数进 history，全文无 test_secs——断链属实。
- perf-73 只管并发链数（现 `read_per_part`＝链数，确无参数表达「跑多久」）；
  perf-75b gate 不改（分位 metric 名不变，`PerfData` 新增字段不进 metric 名）；
  perf-77a 只管时钟源。全 issue 目录 grep `test_secs|稳态|时间窗` 仅命中
  00-r6-io-bench.md（立项决议）、perf-72、72a 与本条，done/todo 零覆盖——
  非重复立项。

### 开发计划

- **冷文件已核实**：`git status -- crate/perf/` 无未提交修改（并行会话热改
  集中在 fkv_io/fkv_io_compio/engine 侧），fkv_bench 可立即开发，无冲突面。
- 步骤：
  1. main.rs 增 `--test-secs <N>`（默认 60 对齐 C++，0＝现状固定遍数回退，
     兼容既有 CI 参数不变）；读阶段改时间窗驱动：链满在飞、完成一批即随机
     取下一批 key，`started.elapsed() >= test_secs` 停。
  2. scan/scan_page 入窗按 C++ 语义（C++ scan 同受 test_secs gate）；
     write 保持次数驱动。
  3. `PerfData` 增 `test_secs` 字段（`#[serde(default)]`，默认 0 表旧口径）
     进 history，旧 perf.json 反序列化不破。
  4. 分位统计本身不动：`LatencyHist` 已是逐样本直方图，窗口只改采样范围。
  5. gate.mjs 无需改；cpp 基线侧（oracle 产 `perf-cpp-*.json`）记录其
     test_secs 值供对比标注。
- 验收在原验收基础上追加：`PerfData` 序列化含 `test_secs` 且旧 history
  反序列化通过。

> 认领（2026-11-05 03:20，本会话子代理开发中）。lane：crate/perf/fkv_bench。

## DONE（2026-11-05）

改动（`crate/perf/fkv_bench/src/main.rs`）：

- 新增 `--test-secs <SECS>`：默认 60，对齐 C++ `simple_bench` 的
  `DEFINE_uint32(test_secs, 60, "read/scan test time")`；`0` = 固定次数
  （历史行为），负值 / NaN 按 0 处理，超大正值饱和到 `Duration::MAX`。
- 点读：`run_read_lane` 在窗口内持续随机读（至少完成一次），直方图与吞吐
  分子均按窗口内完成请求数计；固定次数模式仍是 `rows * read_per_part` 次。
- scan / scan_page：整轮重复直到窗口耗尽（scan 不改变状态），吞吐与分位按
  窗口内全部轮次累计；固定次数模式仍是一轮。
- 写阶段保持次数驱动（对齐 C++ `WriteLoop` 的固定 `write_batchs`），
  `--test-secs` 不放大写入量。
- `PerfData` 新增 `test_secs: f64`（`#[serde(default)]`，旧 history 缺字段
  读作 0），写回 history 记录本次口径。
- 输入参数归拢为 `BenchArgs`，避免 8 参数触发 clippy `too_many_arguments`。
- 文档：`doc/metrics.md` 新增 §6，说明 bench 稳态时间窗、`0` 回退与
  `perf.json.test_secs` 的对比前提。

测试：

- `crate/perf/fkv_bench/src/main.rs` 单测：
  - `test_secs_window_collects_samples`：0.2s 窗口下读 / scan / scan_page
    各阶段均产出完整分位与正吞吐，总耗时 ≥ 0.6s；写指标仍存在（写一遍）。
  - `window_duration_zero_or_invalid_is_fixed_mode`：0 / 负值 / NaN → 0，
    0.2 → 200ms，∞ → `Duration::MAX`。
  - `perf_data_test_secs_back_compat`：旧 `perf.json` 无字段可反序列化，
    新文件往返带 `test_secs`。
- 既有 20 条 bench 用例全部以 `test_secs=0` 保持固定次数口径。

验证：

```
cargo nextest run -p fkv_bench
cargo clippy -p fkv_bench --all-targets -- -D warnings
```

均通过（23/23）。冒烟：

```
cargo run -q -p fkv_bench -- --test-secs 1 --rows 500 --workload read-only ...
# JSON test_secs=1.0，get / scan / scan_page 分位齐全，总耗时约 3 个窗口
cargo run -q -p fkv_bench -- --test-secs 0 --rows 500 --workload read-only ...
# JSON test_secs=0.0，固定次数口径，总耗时约 1.2s（含构建外开销）
```

风险：默认 60 会让 CI 的 read-only / scan 阶段各跑 60s（`write-read` 同时
计量 scan 与 scan_page，即两个窗口）；需要旧口径的调用显式传
`--test-secs 0`。分位统计本身未改，gate.mjs 的字段名不受影响。

## 复核（2026-11-05，开发子代理独立验收）

- 甄别小节行号已按开发前 1237 行快照更正（原 1231 行 / `:741` 系列漂移 4 行），
  符号与结论本身全部核实成立；C++ 对照行号（`simple_bench.cpp:31/186/301-380/
  596-648`）与 `gate.mjs:12/53`、`perf.yml:35` 逐一复核无误。
- 口径决议：任务原文「读/写/scan 各阶段入窗」与 issue 甄别「write 保持次数
  驱动」冲突，按 issue 甄别结论 + C++ 语义（`test_secs` 只 gate read/scan，
  write 按 `write_batchs` 次数）定稿为**写次数驱动**；模块文档、
  `bench_workload` 文档、CLI help 三处已与实现对齐，`doc/metrics.md` §6 在位。
- 验收命令（本任务口径）：
  - `cargo nextest run -p fkv_bench`：23 passed / 0 failed（含 DONE 所列
    3 条新测试，`test_secs_window_collects_samples` 0.618s ≥ 0.6s 下界）。
  - `cargo clippy -p fkv_bench --all-features --all-targets -D warnings`：
    零警告（补齐 DONE 中缺 `--all-features` 的口径）。
- 端到端实跑（rows=max_key=2000，read-only，kv-size 128，batch 100）：
  - `--test-secs 2`：总耗时 6.49s ≈ 3×2s 窗口（读 / scan / scan_page 各自
    判停保证 ≥ 窗口）+ 0.49s 进程与写准备开销；JSON `test_secs=2.0`，
    `get_p99_us=100` 等分位照常输出。
  - `--test-secs 0`：总耗时 0.64s，固定遍数口径与现状一致；JSON
    `test_secs=0.0`。
  - `--help` 确认 `--test-secs <SECS>` 文案标明默认 60、0＝固定次数模式、
    写阶段仍按 `--rows` 次数驱动。
- 「窗口内样本数 ≥ 完成请求数」由构造保证：`get_kv_s` 分子直接取各链直方图
  `count` 之和，每完成请求恰好记一个样本（链回填无丢失），固定次数模式下
  该和恒等于 `rows * read_per_part`，吞吐口径与历史一致。

## 复审（2026-11-05 r7）

**通过。** 参数、窗口语义与测试逐处核实（main.rs 03:37 后无新改动）：

- 参数：`--test-secs <SECS>` CLI 定义（main.rs:277，help 明言写阶段仍按 `--rows`、
  0＝固定次数）；解析 :346-350（默认 `DEFAULT_TEST_SECS = 60.0` :82，`.max(0.0)`
  负值归零）。
- 语义与 C++ 对齐：点读 `run_read_lane`（:696-732）窗口内持续随机读、判停
  `started.elapsed() >= window`（:724），两模式均至少完成一次（:694-695/:709）；
  `window_duration`（:738-744）0/负/NaN→ZERO、正溢出→`Duration::MAX`；scan /
  scan_page 以完整轮为单位在窗口内重复（:874-883、:897+，轮数×每轮条数自洽断言
  :887）；**写阶段固定次数**（:796-799 `while start < rows`，注释明言不随窗口放大）
  ——与 `simple_bench.cpp` `test_secs` 只 gate read/scan、write 按批次一致。
- `PerfData.test_secs: f64`（:209）+ 回兼容测试实证旧 JSON 缺字段读 0.0；
  `doc/metrics.md` §6（:129-141）在位，含 0 回退与跨侧比较前提。
- 测试：`test_secs_window_collects_samples`（:1207-1236，0.2s×3 窗口总耗时
  ≥600ms 下界断言 :1226 + 四阶段分位齐全/全正值 :1228-1235）、
  `window_duration_zero_or_invalid_is_fixed_mode`（:1240-1248，含 NaN/∞ 边界）、
  `perf_data_test_secs_back_compat`（:1252-1274，旧 JSON 反序列化 + 往返）。
  既有用例 BenchArgs 均 `test_secs: 0.0` 保持旧口径。
