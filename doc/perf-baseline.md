# perf-baseline 发布与刷新指引

V6 性能门槛（`.github/workflows/perf.yml` + `scripts/perf_gate.sh`）的比较基准是
`perf-baseline` release 里的 `perf-baseline.json`。本页说明它由谁生成、放在哪、
什么时候必须刷新，以及资产缺失时如何补齐。

## 1. 基线的位置与当前状态

- gate 读取的仓库由 `PERF_BASELINE_REPO` 决定：
  - GitHub Actions：`perf.yml` 显式设为 `${{ github.repository }}`，即
    **本仓（eloqstore2fkv）的 `perf-baseline` release**；
  - 本地直接跑 `scripts/perf_gate.sh gate`：默认回落到 `GITHUB_REPOSITORY`
    （若设置）或 `FKV_REPO`（`webc-site/fkv`）。
- 因此 `webc-site/fkv` 仓里的 `perf-baseline` release 为空**不代表** CI 没有
  基线。本仓当前资产是 2026-11-06T04:25:36Z 产出的 schema 2 基线：
  `bench_commit=50c27aad...`、`data_append_mode=false`、`direct_io=true`，
  固定 `write-read` workload（`key_interval=4` / `del_ratio=4`）。
  `key_interval/del_ratio` 曾被旧二进制记成 grid 默认 `1/0`，已按同一批
  原始指标校正为 C++ 生成器生效值 `4/4`，并留有
  `workload_correction` 字段；指标本身没有重算。
- schema 2 起，基线记录以下审计字段；缺失的 schema 1 基线仍可读，但
  `compare` 只打印 warning 并跳过模式一致性检查：

  | 字段 | 含义 |
  |---|---|
  | `bench_commit` | 产出 baseline 的 fkv SHA（`fkv_bench --commit`） |
  | `data_append_mode` | `false` = PooledFilePages，`true` = Append |
  | `direct_io` | data / segment 文件是否 O_DIRECT |
  | `page_size` | OS / data page 大小（当前固定 4096） |
  | `workload` | 固定 workload 参数（同 runner 比较前提） |

## 2. 谁负责、何时发布

基线**只能**由人工触发的 `publish_baseline=true` 运行产出，不能由周 cron
自动覆盖（避免噪声数字悄悄替换权威基线）。负责人是 perf gate 的维护者；
以下任一变化后必须重新发布：

- fkv 的分配模式 / IO 默认值变化（例如 `a1b2127` 的 data/segment 默认
  O_DIRECT、Pooled 成为默认分配模式）；
- 固定 workload、`fkv_bench` 参数语义或 `KvOption::default()` 变化；
- `PERF_RUNNER_ID` / runner 标签 / 数据盘型号变化；
- `scripts/perf_gate.sh` 的指标集合或阈值口径变化（阈值本身可只改脚本，
  但指标变化需要新基线）。指标**方向**表（见 §6）不写进 baseline schema，
  调整方向不需要重发基线，旧 schema 1/2 基线直接按新方向读取。

特别注意 workload 的 `rng / key_interval / del_ratio`：`fkv_bench` 在
`7e36803` 之前把 `--rng cpp` 的生效值错记为 grid 默认 `key_interval=1 /
del_ratio=0`。用旧二进制发布的基线会与当前 history 的
`rng=cpp, key_interval=4, del_ratio=4` 在 `compare` 的 workload 检查处
失败；遇到这种基线优先按第 2/3 节用当前 fkv 重发。当前资产是在
`7e36803` 之前用真实运行产出后只校正了这两个标签，指标与
`bench_commit` 未改，`compare` 可直接使用。

`write-read` 的 C++ / Rust 并发模型不同：C++ `simple_bench` 同时起写线程和
读线程，Rust 当前是先写完、`reopen`、再读。`write-read` 只用来看 Rust
自身回归，**不要**把它的 `put_kv_s/cpp`、`get_kv_s/cpp` 当作纯写 / 纯读
结论。对外横向对比用 `scripts/perf_local.sh --symmetric` 的 write-only 和
read-only 两对结果；混合负载若必须同口径，需要新增基于
`fkv_runtime::Runtime` 的并发 writer + reader lane。

发布只要求 Linux + 4 KiB 页 + `PERF_NVME_ROOT` 存在。普通 SSD、网络盘、
虚拟盘、容器 overlay 都可以产出 baseline；数字按同 runner 趋势读。

发布前先确认 `.github/workflows/perf.yml` 里的 `PERF_DATA_APPEND_MODE` /
`PERF_DIRECT_IO` 与当前 `fkv_bench` 一致（当前为 `0` / `1`，对应
`KvOption::default().data_append_mode=false` +
`CompioStore::with_direct_io(true)`）。

## 3. 发布步骤

1. 准备 4 KiB 页的 Linux runner + 任意数据盘（本地 SSD、网络盘、虚拟盘、
   overlay 都可以）。runner 标签必须与后续 gate 使用的标签相同，且
   `PERF_RUNNER_ID` 在两次运行间保持一致。
2. 打开 Actions → `perf` → **Run workflow**：
   - `runner_label`：运行 job 的 GitHub runner 标签。留空 = 使用仓库变量
     `vars.PERF_RUNNER_LABEL`，未设置时回落到 `ubuntu-latest`（见
     `.github/workflows/perf.yml` 的 `runs-on` 默认值）。必须与后续 gate
     运行使用同一标签；自定义 runner 用仓库变量统一设置，避免两次运行漂移。
   - `runner_id`：写入 `perf-baseline.json` 的 `runner` 字段，也是 gate 的
     相等检查对象（`baseline.runner != PERF_RUNNER_ID` 时 gate 直接失败）。
     留空 = `vars.PERF_RUNNER_ID`，未设置时回落到
     `github-ubuntu-latest-x86_64`（见 `.github/workflows/perf.yml` 的
     `PERF_RUNNER_ID` 默认值）。**必须与后续 gate 的 `PERF_RUNNER_ID`
     完全一致**，否则按本文发布的基线仍会被 gate 拒绝；自定义 runner 时
     同步设置 `vars.PERF_RUNNER_ID`。
   - `fkv_commit`：要作为基准的 fkv SHA（留空 = fkv 默认分支 HEAD；也
     可用仓库变量 `vars.FKV_COMMIT`）。脚本会 checkout 该 SHA 并从源码
     构建 `fkv_bench`，同时断言 checkout 结果等于该 SHA；
   - `publish_baseline`：勾选 `true`。
3. 该 job 会：
   - `scripts/fetch_eloqstore.sh` 取 pinned eloqstore，构建 `simple_bench`；
   - `scripts/ci/build_rust_bench.sh` 从被审查的 fkv 源码构建 `fkv_bench`；
   - 跑固定 workload，`scripts/perf_gate.sh produce` 写出 schema 2 基线
     （`bench_commit` / `data_append_mode` / `direct_io` 均落盘）；
   - 把 `perf-baseline.json` 覆盖上传到 `PERF_BASELINE_REPO` 的
     `perf-baseline` release，并打印资产名。
4. 发布后核对：

   ```bash
   gh release view perf-baseline -R <本仓> --json assets --jq '.assets[].name'
   gh release download perf-baseline -R <本仓> -p perf-baseline.json -D /tmp/pb
   python3 -I - <<'PY'
   import json
   d = json.load(open('/tmp/pb/perf-baseline.json'))
   print(d['schema'], d['bench_commit'], d['data_append_mode'], d['direct_io'])
   PY
   ```

   期望 `schema=2` 且三个字段与发布时使用的 SHA / 模式一致。

5. 用同一 `fkv_commit` 跑一次 `perf_gate.sh gate`（`publish_baseline=false`），
   确认日志出现 `bench commit asserted: <SHA> == FKV_COMMIT` 且
   `perf gate ok`。

## 4. 资产缺失 / gate exit 78 时

- `PERF_GATE_REQUIRE_BASELINE=1`（CI 固定）下，缺少
  `${PERF_BASELINE_REPO}/perf-baseline/perf-baseline.json` 会
  `skipped-with-error`（exit 78），这是**有意的阻断**，不是要放宽脚本。
- 硬件门只检查 Linux + 4 KiB 页 + `PERF_NVME_ROOT` 存在，不再因
  rotational / 传输层 / 虚拟盘 / 网络盘阻断。任意盘型的结果都按同 runner
  趋势读。
- `PERF_NVME_ROOT` 留空时 prepare step 回落到 `$RUNNER_TEMP/fkv-perf-data`；
  `/tmp` 可能是 tmpfs，建议显式设 `vars.PERF_NVME_ROOT` 指向真实数据目录。
- 先确认读取的仓库是否弄错：CI 读本仓 `perf-baseline`，本地默认可能读
  `webc-site/fkv`（那边确实是空的）。本地复现 CI 时显式设
  `PERF_BASELINE_REPO=<本仓>`，或直接在 Actions 上跑。
- 确认仓库正确但仍无资产：按第 2/3 节补跑 `publish_baseline=true`。
- 不要在 CI 里改 `PERF_GATE_REQUIRE_BASELINE=0` 绕过；该开关只用于临时排障。

## 5. 本地 produce

`scripts/perf_local.sh --rebuild` 也会调用 `produce`，产出的 JSON 带
schema 2 字段。现在 `produce` 不再区分权威 / 非权威硬件，任意 4 KiB Linux
盘都能写出 baseline JSON；差异只体现在 runner id 与硬件日志里。

## 6. 指标方向与门槛参与

`compare` 不再使用「非延迟即吞吐」的默认，而是按 `scripts/perf_gate.sh` 里的
`classify_metric` 显式定方向（源码即口径，改表要同步本节）。
方向是 **gate 侧策略**：baseline schema 不记录方向字段，所以调整方向不要求
重发 `perf-baseline.json`；baseline 与 current 仍必须是同 runner / 同模式 /
同 workload。

| 类别 | 指标名匹配（示例） | 方向 | 硬门槛 | 容差 |
|---|---|---|---|---|
| 吞吐 / 速率 | `put_kv_s`、`delete_kv_s`、`put_mib_s`、`put_batch_s`、`get_kv_s`、`scan_kv_s`、`scan_page_s`、`compact_file_s` | 越高越好 | 是 | `PERF_MAX_REGRESSION_PCT`（默认 20%） |
| 延迟 | `reopen_us`、`compact_total_us`、`put_p99_us`、`get_p95_us`、`get_avg_us`、`get_window_p50_us` 等含 `avg/p50/p90/p95/p99/max` 的 `*_us` | 越低越好 | 是 | `PERF_MAX_LATENCY_REGRESSION_PCT`（默认 100%） |
| 进程 RSS | `rss_peak_kb` | 越低越好 | 是 | `PERF_MAX_RSS_REGRESSION_PCT`（默认 20%） |
| 资源 / 计数 / 窗口 | `store_bytes*`、`store_file_num*`、`store_bytes_before/after_compact`、`cache_*`、`compact_store_bytes_before/after`、`io_*_num`、`proc_io_*`、`write_amp`、`space_amp_nominal`、`write_at_per_batch`、`sync_per_batch`、所有 `*_window_*`、`file_num`、`*_cnt`、`*_count`、`*_bytes`、`*_ratio`、`*_deleted` | 只审计 | 否 | 无 |
| 配置回显 | `ttl_ratio`、`ttl_ms`、`compact_pages_per_file` | 只审计 | 否 | 无 |
| `--repeat` 聚合变体 | `<metric>_min` / `<metric>_max` | 只审计 | 否 | 无（中位数才是门槛值） |
| 已知噪声 | `*_max_us`、`*_p999_us`、`*_p9999_us` | 跳过 | 否 | 不打印 |
| 未登记 | 其它任何名字 | 只审计 + warning | 否 | 无 |

`gxhash_4096B_gib_s` 已从 `fkv_bench` 默认输出和 gate 中退役。它是 4 KiB
单块的进程内哈希微基准，既不是 KV 引擎路径，也不是 C++ `simple_bench`
指标；旧基线里若仍带该键，`compare` 会打印 `retired metric skipped` 并
跳过，不要求重发基线。

资源 / 计数类为什么只审计、不设「越低越好」门槛：

- `store_bytes` 取决于数据集合、文件布局与后台 GC 时机，单次点值方差大。
  若反过来设「不得变大」阈值，空间回收 / 压缩带来的真实改善会被判红，
  等于把本 issue 的假红换个方向重演；它更适合看趋势而不是单次硬门槛。
- `io_*_num` / `proc_io_*` 是路径相关计数：fd 复用、sync 合并会让计数下降，
  但下降不一定代表更好（例如 `io_data_sync_num` 变少可能意味着持久化语义
  变化）；`proc_io_*` 还是进程累计（含二进制 / 依赖读取），`--repeat>1` 时
  更是多次运行累计，没有稳定的单次口径。
- `*_window_*` 是 1 秒窗口的抖动 / 阶段时长代理；单窗口可能赶上一次 fsync
  长尾，即使整段吞吐不变也会大幅波动。吞吐已由整段 `*_kv_s` / `*_qps`
  覆盖，窗口指标只审计。

未登记指标（例如未来 fkv_bench 新增的名字）**不会**默认按吞吐判：`compare`
把它按 `info` 输出并在 stderr 打印 `WARNING: 方向表未登记的指标...`，不进入
`regressions`。要让它参与硬门槛，先在 `classify_metric` 登记方向，再同步
本节与 selftest。输出末尾的 `gated=/informational=/unknown=` 给出三类计数。

`*/cpp` 比值门槛只对「越高越好」的吞吐指标生效；延迟 / RSS / 资源 / 计数 /
未登记指标不参与比值判据。
