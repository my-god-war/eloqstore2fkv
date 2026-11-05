
## 复审（2026-11-04，第三轮）

**已在 `.github/workflows/perf.yml` 中落地（由其它会话实现），逐条核对无误后关闭。**

| 方案项 | 状态 | 证据（perf.yml） |
|---|---|---|
| 1. 参数抽到 job 级 `env`，`vars.PERF_BENCH_*` 可覆盖，步骤只引用 env | 已实现 | `bench` job `env:` 三条：`PERF_ROWS: ${{ vars.PERF_BENCH_ROWS \|\| '20000' }}`、`PERF_BATCH_SIZE: …\|\| '100'`、`PERF_WORKLOAD: …\|\| 'write-read'`；bench 步骤只用 `"$PERF_WORKLOAD" / "$PERF_ROWS" / "$PERF_BATCH_SIZE"`，无重复字面量。 |
| 2. 新增 fetch cpp baseline 步骤，缺失不失败、输出 warning 与 `present=false` | 已实现 | `- name: fetch cpp baseline` (`id: cpp_baseline`)：`gh release view perf-baseline` + `gh release download … --pattern "perf-cpp-${{ matrix.arch }}.json"`；失败分支 `::warning::no cpp baseline asset …`，末行 `echo "present=$present" >> "$GITHUB_OUTPUT"`。 |
| 2b. 有 asset 时随 rust json 一起上传 artifact | 已实现 | `actions/upload-artifact@v7` 的 `path:` 同时列 `perf-${{ matrix.arch }}.json` 与 `perf-cpp-${{ matrix.arch }}.json`。 |
| 3. 不改 `merge.mjs` | 已遵守 | `web/scripts/merge.mjs:6` dedup key = `[commit, platform, metric, impl]`，rust / cpp 天然并存；本次未改该文件。 |

### 验收实测

1. **YAML 解析**：`ruby -ryaml -e 'YAML.load_file(...)'` → `perf.yml OK jobs=["bench","publish"]`（无 pyyaml，用 ruby）。
2. **缺 cpp asset 时步骤不失败**：用返回 1 的 `gh` 桩 + `bash -e`（GitHub 默认 shell）复跑该步骤 → 退出码 **0**，输出 `::warning::no cpp baseline asset …`，`GITHUB_OUTPUT` 写入 `present=false`。
3. **有 cpp asset 时 merge 后出现 `impl=cpp`**：按 workflow 产物布局造 `perf-artifacts/{perf-x86_64.json, perf-cpp-x86_64.json}`，`node web/scripts/merge.mjs perf.json perf-artifacts` → `2 runs`，`impls=['cpp','rust']`，`baseline` 保留。
4. **脚本无机器相关硬编码数值**：bench 只引用 env；`BENCH_ROOT` 走 `vars.PERF_BENCH_ROOT`；阈值走 `vars.PERF_GATE_*`。

### 说明（与原文的偏差）

- env 变量名实际为 `PERF_ROWS` / `PERF_BATCH_SIZE` / `PERF_WORKLOAD`（而非 `PERF_BENCH_*`），
  但**来源**仍是 `vars.PERF_BENCH_*`，单一真源与覆盖能力与方案等价，不做改名（避免无谓改动）。
- 生产者侧（oracle 环境跑 `simple_bench` 并上传 release asset `perf-cpp-<arch>.json`）仍缺位：
  `oracle.yml` 只 build 并把 `simple_bench` 传为 `oracle-build` artifact，不产出 `perf-cpp-*.json`，
  属 `perf-75` / `01` 的剩余范围，本条（摄取侧）不覆盖。

无需改动，本 issue 关闭。

## 复审（2026-11-05 r6）

通过（对第三轮复审小节逐项再核）。

- `.github/workflows/perf.yml:35-37` `PERF_ROWS/PERF_BATCH_SIZE/PERF_WORKLOAD`
  env（源 `vars.PERF_BENCH_*`，默认 20000/100/write-read），bench 步骤只引用
  env（`:63-64`）。
- `:70-82` `fetch cpp baseline` 步骤（id cpp_baseline）：`gh release view
  perf-baseline` + download `perf-cpp-<arch>.json`，缺失 `::warning::` 不阻断，
  present 输出；`:90` artifact 同时上传 cpp json。
- `web/scripts/merge.mjs` 未改、dedup key 含 impl，与记录一致。
