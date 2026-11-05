# perf-76 看板 ratio 无方向区分、默认 metric 是 gxhash、无门槛展示

## 位置
- Rust: `web/src/App.svelte:15-18`（metric/platform 默认取首个）、`App.svelte:32-35`（ratio 计算）、`App.svelte:101-105`（ratio 卡片）、`App.svelte:113-129`（metric 选择器）
- Rust: `crate/perf/fkv_bench/src/main.rs:154-157`（gxhash run 最先 push）
- C++: `benchmark/simple_bench.cpp:219-274`（吞吐与延迟分位分开输出）

## 现状
- `metric_li = [...new Set(runs.map(r => r.metric))]`（`App.svelte:15`），默认 `metric = metric_li[0]`（`App.svelte:17`）。
- fkv_bench 最先 push `gxhash_4096B_gib_s`（`main.rs:154-157`），故看板默认展示的是哈希吞吐，而非 workload 指标。
- ratio 统一 `rust_latest.value / cpp_latest.value`（`App.svelte:33`），无方向区分。
- 无门槛线、无达标/未达标着色。

## C++ 实现 / 目标口径
- C++ `simple_bench` 吞吐（kvs/s、MB/s）与延迟分位（p50/p99/µs）分开输出（`simple_bench.cpp:219-274`）。
- todo.md 验收：吞吐类 ratio ≥ 0.85/0.9（越高越好），延迟/RSS 类 ratio ≤ 1.2/1.1（越低越好）。

## 差异与风险
- **默认 metric 错误**：首访看板看到 gxhash 哈希吞吐 ratio，与存储引擎性能无关，误导。
- **ratio 方向不分**：吞吐 ratio 0.9 是达标（≥0.85），延迟 ratio 0.9 却意味着比 C++ 快 10%（应 ≤1.2）。统一 `rust/cpp` 比值不区分方向，无法判定达标。
- **无门槛展示**：看板不显示阈值，人工无法快速判断是否达标。

## 对齐建议
- metric 按类别排序：workload 吞吐/延迟优先，gxhash 放最后或单独 tab。
- ratio 增加方向语义：吞吐类显示「rust/cpp」并标绿 when ≥ 阈值；延迟/RSS 类标绿 when ≤ 阈值。
- ratio 卡片增加门槛线与达标状态（pass/fail）。
- metric 元数据（方向、阈值、单位）写入 perf.json 或前端常量表。

## 性能优化建议
- 前端可缓存 perf.json，避免每次 fetch；但看板数据量小，非瓶颈。

## 严重度
P1（口径不一致/功能缺失）

## DONE（2026-11-04）

三条现状全部处理（看板口径逻辑抽成纯函数模块，可自测）：

1. **默认指标不再是 gxhash**：新增 `web/src/lib/perf.js`
   - `metricRank`/`sortMetricLi`：微基准（`gxhash*`）排在 workload 指标之后，
     同方向指标相邻、按名字排序；
   - `defaultMetric`：优先 `PRIMARY_METRIC_LI`（`put_kv_s`/`get_kv_s`/`scan_kv_s`/
     `scan_page_s`/`put_batch_s`），都没有才退回排序后的首个。
   `App.svelte` 的 `metric_li` 与默认值改走这两个函数。
2. **ratio 有方向**：方向由单位判定（`isThroughput`：`*/s` 为吞吐，越大越好；
   `us`/`kB` 为代价，越小越好），卡片副标题输出
   「越高越好/越低越好 · 门槛 ≥0.85 或 ≤1.2 · 达标/未达标」，缺 Rust 或 C++ 样本时
   显示「缺 C++ 或 Rust 样本」且不做判定。
3. **门槛与达标着色**：`ratioPass` 按吞吐 ≥ `GATE_THROUGHPUT_MIN`(0.85)、
   代价 ≤ `GATE_COST_MAX`(1.2) 判定；ratio 卡片加 `class:Pass` / `class:Fail`，
   `src/styl/_.styl` 给两种状态绿/红底色与数值色。
4. i18n：`higherBetter` / `lowerBetter` / `gate` / `gatePass` / `gateFail` / `noSample`
   中英各一套（`src/i18n.js`）。

验证：
- 新增 `web/sh/perf.test.js`（node 内置 test runner，`bun run test`）：
  4 passed（微基准排序、默认指标、方向与门槛判定、ratio 文案三种分支）；
- `bun run build`（`node build.js` + `vite build`）通过：114 modules transformed，
  Svelte 编译通过 → 模板里的 `class:Pass`/`class:Fail` 与响应式表达式合法；
- 产物校验：`style.css` 含 `#f2fbf3`/`#fff3f1`（Pass/Fail 底色），
  `dist/assets/*.js` 含 `higherBetter`/`lowerBetter`/`noSample` 与门槛常量 1.2。
- 注：`index.html` / `style.css` / `dist/` 均在 `web/.gitignore` 内，构建产物不入库。

未做（留有依据，不是遗漏）：
- 门槛只实现**单档**主门槛（吞吐 0.85 / 代价 1.2）；todo.md §7 里
  「0.85 与 0.9」「1.2 与 1.1」的两档（软/硬）没做，避免在没定死验收档位前把 UI 写死。
- 微基准未单独 tab（用排序+默认值解决误导问题）；若后续要看 R²/分布再拆面板。
- `rss_peak_kb`（perf-80）目前只有 Linux CI 产出，RSS ratio 复用代价方向的判定，
  本地 macOS 跑 bench 时不会出现该 metric。

## 复审（2026-11-05 r6）

通过。

- `web/src/lib/perf.js:7 PRIMARY_METRIC_LI`、`:10 GATE_THROUGHPUT_MIN=0.85/
  GATE_COST_MAX=1.2`、`:14-18 isThroughput/ratioPass/gateText/metricRank`、
  `:21-24 sortMetricLi/defaultMetric`。
- `web/src/App.svelte:6` import、`:39 pass = ratioPass(ratio, unit)`、
  `:105 class:Pass/class:Fail` 着色。
- `web/sh/perf.test.js` 存在（微基准排序、默认指标、方向与门槛判定、文案分支）。
