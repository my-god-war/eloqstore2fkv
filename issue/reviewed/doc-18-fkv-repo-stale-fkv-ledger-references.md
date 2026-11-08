# doc-18 fkv 仓对已迁出的 `fkv_ledger` crate 的失效引用（`test.sh` 注释误述 + docker-compose `--exclude` 空转）

- 领域：文档账本一致性 / 失效引用（fkv 仓 × 已迁到 issue 仓的工具 crate）
- 优先级：P4（纯注释 / 本地容器配置；cargo 对悬空 `--exclude` 宽容，零行为影响）
- 置信度：高（workspace 成员枚举 + `cargo metadata` 实验 + 全仓 grep，三处引用可复现）

## 现象

fkv workspace 里**没有** `fkv_ledger` 这个成员（`fkv_ledger` 现只存在于 issue 仓
`eloqstore2fkv/tool/fkv_ledger`），但 fkv 仓仍有三处引用它：

```text
doc/architecture/01-overview.md:61   ./test.sh        # cargo nextest（+ fkv_ledger 单测）
docker/docker-compose.yml:55          command: ["cargo","nextest","run","--workspace","--all-features","--exclude","fkv_ledger"]
docker/docker-compose.yml:70          command: ["cargo","bench","--workspace","--exclude","fkv_ledger"]
```

1. **注释误述**：`01-overview.md:61` 声称 `./test.sh` 会跑「`+ fkv_ledger` 单测」。
   实测 `test.sh` 只有两步——`bash scripts/ci/check_layers.sh` 与
   `cargo nextest run --failure-output immediate --status-level slow
   --final-status-level slow --show-progress=none`（workspace 全量）。它在
   fkv workspace 内运行，**不可能**跑到 issue 仓的 `fkv_ledger`，该注释陈述错误。
2. **docker `--exclude` 空转**：`docker/docker-compose.yml` 的 `test` / `bench`
   两个 service 都对一个不存在的 crate 传 `--exclude fkv_ledger`。

## 证据

- workspace 成员枚举（fkv 冻结 worktree `/tmp/r29/fkv` HEAD `b3b37d1`）：
  `ls -d crate/*/*/` 共 39 个 crate，无 `fkv_ledger`；
  `grep -rn fkv_ledger . --exclude-dir=target --exclude-dir=.git` 仅命中上述三处。
- `Cargo.lock` 无 `fkv_ledger`（`grep -n fkv_ledger Cargo.lock` → 空），
  即它既非成员也非依赖。
- cargo 宽容性实测：`cargo test --workspace --exclude fkv_ledger --no-run`
  → exit 0、无输出（悬空 `--exclude` 被静默忽略，不报错、不告警），故
  docker-compose 的两处实为 no-op。
- 对照：issue 仓 `eloqstore2fkv/tool/fkv_ledger` 才是该 crate 的现居地，
  其 `oracle.yml` 用 `cargo test -p fkv_ledger` 单独覆盖；fkv 仓无对应成员。
- 溯源：fkv 主线为周期性 re-init（单条 `init` 提交，`engine.yml` 注释亦载明），
  `fkv_ledger` 随某次 re-init 迁出 fkv、残留在 doc / docker 引用未清。

## 影响

- 无功能 / 行为影响：`--exclude` 空转，docker-compose 又仅用于本地开发调试
  （README「`docker/docker-compose.yml` 仅用于本地开发与调试」）。
- 但制造认知噪声：`01-overview.md:61` 让读者以为 `test.sh` 覆盖 `fkv_ledger`
  单测，实际不然；docker-compose 暗示该 crate 属于本 workspace。

## 建议

- `doc/architecture/01-overview.md:61` 去掉「（+ fkv_ledger 单测）」，或改为
  准确描述（如 `# cargo nextest 全量（每测试独立进程）`）。
- `docker/docker-compose.yml:55,70` 删除两处 `"--exclude","fkv_ledger"`
  （crate 不在 workspace，无需排除）。

## 验收方式

```bash
cd /tmp/r29/fkv
grep -rn fkv_ledger doc/architecture/01-overview.md docker/docker-compose.yml   # 期望 0
cargo test --workspace --exclude fkv_ledger --no-run && echo "cargo 容忍悬空 exclude（对照）"
```

## 处置记录

- 2026-11-08 第 39 轮结构 / 文档账本域换面 spot check（`sh/` + `.github/workflows/`
  + `docker/`）发现；fkv 冻结 worktree `/tmp/r29/fkv` HEAD `b3b37d1` 取证。

## 实现记录（2026-11-08，coordinator 直修）

- 修复 fkv commit **`ac19554`**：
  - `doc/architecture/01-overview.md:61` 注释
    `./test.sh # cargo nextest（+ fkv_ledger 单测）`
    → `./test.sh # 分层/行数门禁 + cargo nextest（workspace 全量）`
    （`fkv_ledger` 已迁至 issue 仓，fkv 的 `test.sh` 不跑它）；
  - `docker/docker-compose.yml:55,70` 两处悬空 `--exclude fkv_ledger`
    （cargo 静默忽略的 no-op）→ 改指 `fkv_nvme_test`（与 CI / `./test.sh`
    的既有排除口径一致；该成员存在，`cargo metadata` 核实 40 个成员含
    `fkv_nvme_test`、不含 `fkv_ledger`）。
- 验证：`grep -rn fkv_ledger doc/ docker/ sh/ test.sh` → 零命中；
  `cargo metadata` 成员核实；改动为注释 + 本地容器配置（零行为影响）。
- 未立案观察（交协调者裁量，不属本条）：fkv `ci.yml` 未接入
  `scripts/ci/check_layers.sh`（分层 / 行数门禁只在 `./test.sh` 覆盖）——
  AGENTS.md 与 README 均把 `./test.sh` 定为开发门禁、把 ci.yml 描述为
  fmt/clippy/全量/S3，倾向 by design；本轮记录不立项。

## 复审记录（2026-11-08，第 40 轮结构域）

- 判定：**达标**，移入 reviewed/。fkv 仓 `grep -rn fkv_ledger` 零命中；
  docker compose 与 `ci.yml` 排除口径同形；`cargo metadata` 成员核实
  （40 成员含 `fkv_nvme_test`、不含 `fkv_ledger`）。措辞勘误：记录所称
  「与 `./test.sh` 口径一致」不精确——`test.sh` 不传 `--exclude`（靠
  `fkv_nvme_test` 自跳过），**实践口径一致、非缺陷**。
