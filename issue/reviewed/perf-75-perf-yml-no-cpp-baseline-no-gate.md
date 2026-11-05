# perf-75 perf.yml 只跑 rust，无 C++ 基线数据管线，相对比值门槛不生效

## 位置
- Rust: `.github/workflows/perf.yml:54-59`（bench 步骤只 `--impl rust`）、`perf.yml:60-64`（上传 artifact）、`perf.yml:83-89`（merge 历史）
- Rust: `web/scripts/merge.mjs:6`（dedup key 含 `impl`）、`web/src/App.svelte:32-35`（ratio 计算）、`web/src/App.svelte:101-105`（ratio 卡片）
- C++: 无对应 CI（eloqstore 不跑 fkv perf.yml）

## 现状
- perf.yml bench 步骤固定 `--impl rust`（`perf.yml:56`），**从不跑 `--impl cpp`**。
- merge.mjs 按 `[commit, platform, metric, impl]` 去重合并（`merge.mjs:6`），但 artifact 里只有 rust run。
- web `App.svelte` 计算 `ratio = rust_latest.value / cpp_latest.value`（`App.svelte:32-35`），`cpp_latest` 恒为 null → ratio 恒显示 `—`（`App.svelte:103`）。
- perf.yml 无任何步骤校验 ratio 是否达到验收门槛（p99 ≤ 1.2×、吞吐 ≥ 0.85×、scan ≥ 0.9×、RSS ≤ 1.1×）。

## C++ 实现 / 目标口径
- todo.md D15：相对比值做 CI 门槛。
- todo.md 验收标准第 6 条：同机对比 eloqstore，四个比值阈值。
- C++ `simple_bench` 在本地或 oracle 环境跑，产出 `--impl cpp` 数据需人工或独立 pipeline 注入 perf.json。

## 差异与风险
- **C++ 基线数据从未进入 perf.json**：看板 ratio 永远是 `—`，D15「相对比值做 CI 门槛」**未落地**。
- **无门槛检查**：perf.yml 只合并推送，不 fail；即使 rust 数据退化，CI 仍绿。验收标准无法在 CI 强制。
- dedup key 含 `commit`：rust 与 cpp commit 不同不会冲突，但因无 cpp run，ratio 无法计算。
- 即使人工注入 cpp run，merge 按 `commit|platform|metric|impl` 去重，rust/cpp 不同 commit 不会互相覆盖，但 ratio 取各自 `latest`，需保证同 platform 同 metric。

## 对齐建议
- perf.yml 增加 cpp bench job：在 oracle runner 上编译 eloqstore `simple_bench`，用相同参数跑 `--impl cpp`，产出 cpp run。
- 或独立 `oracle-perf.yml` 跑 C++，结果与 rust 一起进 `perf-artifacts` 目录供 merge。
- merge 后增加门槛校验步骤：对每 metric 计算 ratio，与阈值比对，不达标则 `exit 1` 使 CI 失败。
- 门槛方向需区分：吞吐类 ratio ≥ 阈值（0.85/0.9），延迟/RSS 类 ratio ≤ 阈值（1.2/1.1）。

## 性能优化建议
- 门槛校验脚本可复用 `fkv_metrics` 直方图输出，避免二次解析。
- cpp/rust 必须同 runner 同参数，否则 ratio 无意义；perf.yml 当前 rust 用 `--rows 20000 --batch-size 100`（`perf.yml:58`），与 C++ README 参数（batch 20000）不同，需统一。

## 严重度
P0（测量失真/验收无法达标）

## 审查结论（2026-11-04）

核验结果：成立。

- `perf.yml:54-59` 只跑 `--impl rust`，确实没有 cpp run；`web/scripts/merge.mjs` 按 `commit|platform|metric|impl` 去重，缺少 cpp 时 `App.svelte` 的 ratio 恒为 `—`。
- `perf.yml` 确实没有任何 ratio / 阈值校验步骤，rust 退化不会 fail。
- 参数口径：perf.yml 用 `--rows 20000 --batch-size 100`，C++ README 用 batch 20000；语义对齐归 perf-74，本 issue 只把参数抽成 workflow 级唯一来源，不改语义。
- 「p99 ≤ 1.2×、RSS ≤ 1.1×」当前 bench 尚未输出 p99 / RSS（perf-72、perf-80 未完成），门禁脚本按指标名前缀预留方向，先对现有吞吐指标生效，不写死未来字段名。
- 限制：本仓 `cpp/eloqstore` 为空（非 submodule、未 checkout），`simple_bench` 无法在本地/CI 现场构建验证；`oracle.yml` 只构建并上传 `oracle-build` 产物，输出格式未在本仓定义。故 cpp 基线采用「外部产物 + release asset 注入」管线，不在 perf.yml 里猜 simple_bench CLI/输出格式。

拆分：

- `perf-75a`：perf.yml 注入 `--impl cpp` 基线数据、统一 bench 参数来源。
- `perf-75b`：新增 ratio 门禁脚本并在 publish 阶段执行，失败输出可诊断信息。

遗留（不在本 issue 内闭环）：真正产出 `perf-cpp-<arch>.json` 需要 oracle 环境跑 `simple_bench` 并上传 release asset；`cpp/eloqstore` 补齐后才可做端到端验证。

## 复审（2026-11-05 r6）

通过（拆分条 75a/75b 均已落地，见各自 r6 小节）。

- 拆分决议（摄取侧 75a / 门禁侧 75b）与现状吻合；本条作为总纲关闭合理。
  C++ 基线生产侧（oracle 环境跑 simple_bench 上传 release asset）仍缺位，
  已在 75a「说明」与遗留中如实记录。
