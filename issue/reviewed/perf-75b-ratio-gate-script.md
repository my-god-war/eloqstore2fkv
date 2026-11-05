# perf-75b 新增 ratio 门禁脚本并在 CI 执行

## 位置
- 新增 `web/scripts/gate.mjs`
- `.github/workflows/perf.yml`（publish job，merge 之后）

## 方案
1. `gate.mjs` 读合并后的 `perf.json`，按 `platform + metric` 取 rust / cpp 各自 latest run，计算 `ratio = rust / cpp`。
2. 指标策略表（方向 + 阈值）：
   - 吞吐类 `put_kv_s` / `get_kv_s` / `put_batch_s`：`min 0.85`
   - `scan_*`：`min 0.9`
   - 预留延迟类（`p99*` / `latency*`）：`max 1.2`
   - 预留内存类（`rss*` / `mem*`）：`max 1.1`
   - `gxhash_*` 与实现无关，跳过；未知指标只报告不判定。
3. 诊断输出：每个 ratio 一行表格（rust / cpp / ratio / 阈值 / 结论），缺失基线与单位不一致单独标注；写 `$GITHUB_STEP_SUMMARY`。
4. 退出码：任一 ratio 越界 → 1；缺 cpp 基线时受 `PERF_ALLOW_MISSING_BASELINE` 控制（1=仅 warning，0=失败），便于基线管线落地前渐进接入。
5. perf.yml publish job：merge → gate（`continue-on-error`）→ 推 gh-pages（`always()`）→ 上传 gate 报告 → 最终按 gate 结果 fail。数据先落盘，避免回归数据丢失。
6. 顺带提示 rust 自回归（对比同 platform/metric 上一 rust run，默认仅 warning，避免噪声直接红）。

## 验收
- 构造含 cpp run 的 perf.json：达标通过、越界退出 1、方向判断正确（min/max）。
- 缺 cpp run：`PERF_ALLOW_MISSING_BASELINE=1` 退出 0 且输出 warning；`=0` 退出 1。
- 单位不一致、未知指标有明确提示。
- `perf.yml` YAML 解析通过；无机器相关硬编码数值。

## DONE（2026-11-04）

实现：

- 新增 `web/scripts/gate.mjs`：读合并后的 perf.json，按 `platform + metric` 取 rust / cpp latest（同 date 取后者），`ratio = rust / cpp`。
- 策略表：`scan_*` → `min 0.9`；`put_kv_s` / `get_kv_s` / `put_batch_s` → `min 0.85`；`p*_` / `latency*` → `max 1.2`；`rss*` / `mem*` → `max 1.1`；`gxhash_4096B_gib_s` 跳过；未知指标只记录。阈值可用 `PERF_GATE_*` env 覆盖。
- 诊断：markdown 表格（rust / cpp / ratio / 阈值 / 结论），缺失基线、单位不一致、cpp 值 0、cpp 基线过期、`PERF_COMPAT_BASELINE` 与 cpp commit 不一致均给出具体行；写 `$GITHUB_STEP_SUMMARY` 与 `--report` 文件；同时输出 `::error::` / `::warning::` 注解。
- 缺 cpp 基线由 `PERF_ALLOW_MISSING_BASELINE` 控制：1 告警、0 阻断；rust 自身回退（默认 20%）仅告警，可 `PERF_SELF_REGRESSION_FAIL=1` 转阻断。
- `.github/workflows/perf.yml` publish job：拉取 `gate.mjs` 与 `COMPAT_BASELINE` → merge → `ratio gate`（`continue-on-error`）→ 推 gh-pages（`always()`，数据先落盘）→ 上传 `perf-gate.md` → `enforce gate` 按 gate 结果 `exit 1`。

验证（本地 dry-run，fixture 在 scratch 目录）：

| 场景 | 结果 |
| --- | --- |
| 全部达标（含 p99 / rss） | exit 0 |
| put 0.70、scan 0.80、p99 1.30、rss 1.20 | exit 1，四行 FAIL |
| cpp 单 metric 单位不一致 | exit 1 |
| 缺 cpp，`PERF_ALLOW_MISSING_BASELINE=1` | exit 0，warning 列出缺失项 |
| 缺 cpp，`=0` | exit 1 |
| cpp commit 与 `PERF_COMPAT_BASELINE` 不符 | exit 1 |
| rust 自回退 50% | exit 0 + warning |
| 空 runs / 参数顺序调换 | exit 0 / 报告文件正确生成 |

- `node --check web/scripts/gate.mjs` → 通过；`ruby -ryaml` 解析 perf.yml → 通过；`cargo check -p fkv_bench` → exit 0。

遗留：

- `PERF_GATE_REQUIRE_BASELINE` 默认未设（仅告警）。oracle 基线资产上线后应把仓库变量设为 `1`，缺基线即阻断。
- eloqstore 基线资产产出需 oracle 环境，本仓无法端到端跑通 CI 门禁；阈值方向（延迟 / RSS）已按名字前缀预留，等 perf-72 / perf-80 落地后自动生效，无需改脚本。

## 复审（2026-11-05 r6）

通过。

- `web/scripts/gate.mjs` 存在：策略表 scan_* min 0.9 / put_kv_s·get_kv_s·
  put_batch_s min 0.85 / p\d+·latency max 1.2 / rss·mem max 1.1 /
  gxhash_4096B_gib_s 排除；`PERF_GATE_*`、`PERF_ALLOW_MISSING_BASELINE`、
  `PERF_SELF_REGRESSION*` env 覆盖（`:34-41`）。
- `perf.yml:124-132` `ratio gate` 步骤（id gate，env 注入阈值与
  PERF_GATE_REQUIRE_BASELINE 翻转逻辑）、`:148` gate report 上传；
  publish job 拉取 gate.mjs（`:110-111`）后 merge → gate → 推 gh-pages →
  enforce 的顺序与 md 一致。
