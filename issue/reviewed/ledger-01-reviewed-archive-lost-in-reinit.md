# ledger-01 re-init 丢失 issue/reviewed 历史档案：账本引用断链 1209 处 / 基线压行漂移 / 看板过期

- 领域：账本 / 仓库完整性（协调者直修，非 fkv 代码问题）
- 优先级：P1（三账本门禁全红：issue links 1200 missing、baseline drift 45 项、
  progress board 不一致——结构域门禁在本条修复前不可能绿，收敛判定被阻断）
- 置信度：高（命令输出可直接复现）

## 现象

2026-11-07 19:32 issue 仓被重 init 为单条提交 `4759859`（本仓第 1 次、fkv 仓
第 5 次历史重写）。init 快照只保留了 `issue/{new,done,todo}` 的当前在途文件，
**不含 `issue/reviewed/`（与 `issue/reject/` 目录）**，而：

1. `COMPAT_BASELINE` 的登记条目引用 26 处 `issue/reviewed/*.md`；
2. `doc/*.md` 引用 30 处；
3. 44 份 `fixtures/**/meta.txt` 的 `baseline=` 压行内嵌 COMPAT_BASELINE 全文，
   连带 1144 处引用——合计 **1209 处引用、29 个不同文件名**全部 MISSING
   （`bash scripts/check_issue_links.sh --check` exit 1）；
4. `issue/done/` 里的 sem-67 在 re-init 后登记了 COMPAT_BASELINE（77386→
   79073 chars），44 份 meta 与 `doc/progress.md` 未随之刷新
   （`check_baseline_sync --check` 45 项 MISMATCH）；
5. `check_progress_board --check` 红（看板 234 reviewed vs 实际目录 0）。

旧文件内容不可恢复（re-init 后无 dangling 对象，`git fsck --unreachable` 空）。

## 建议（已实施）

1. **存档桩**：对 29 个被引用文件名在 `issue/reviewed/` 重建存档桩——桩内
   明确声明「原始甄别 / 实现 / 复审记录在 re-init 中丢失，本桩仅恢复账本引用
   完整性」，并指向存活证据（COMPAT_BASELINE 对应条目 + `00-index.md` 轮次
   记录摘录）。不伪造任何原始内容。
2. `check_baseline_sync.sh --fix` 刷新 44 份 meta + progress.md 压行。
3. `cargo run -p fkv_ledger -- report --out doc/progress.md` 重生成看板。

## 验收方式

```bash
bash scripts/check_baseline_sync.sh --check   # 44/44 ok, 79073 chars
bash scripts/check_issue_links.sh --check     # 0 missing / 0 wrong-state
bash scripts/check_progress_board.sh --check  # report diff ok
```

三条全部 exit 0（2026-11-07 实测：1238→1283 引用 0 缺失、44/44、看板一致）。

## 处置记录

- 2026-11-07 第 29 轮结构域前置修复（协调者直修；fkv 历史重写属已登记的
  环境问题，本条只修账本引用完整性，不涉及 fkv 代码）。
- 遗留声明：29 个存档桩是**引用完整性桩**，不是原始复审记录；后续轮次若
  复审发现需要引用原始内容，以 COMPAT_BASELINE 条目与 `00-index.md` 轮次
  记录为准。reject 目录 io-01 / pooled-18 两条终态记录同样丢失，当前无任何
  文件引用它们（grep 零命中），不重建桩。

## 复审记录（2026-11-07，第 30 轮结构域）

- 判定：**达标**，移入 reviewed/。三账本只读复跑全绿（baseline 44/44 /
  80845 chars、links 1329 引用 0 缺失 0 状态错位、看板 {todo 0, new 0,
  done 3, reviewed 30, reject 0} 与目录实数一致 + report diff ok）；第 29
  轮遗留的看板自指 drift 已随 `52423db` 闭合；存档桩抽查（sem-58 / io-28
  / sem-64 / sem-42 / sto-51 共 5 桩）均诚实声明丢失 + 指向存活证据，
  sem-58 摘录与 00-index.md:1452 逐字一致。
