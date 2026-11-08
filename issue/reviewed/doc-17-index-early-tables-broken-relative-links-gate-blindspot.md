# doc-17 `issue/new/00-index.md` 早期「发现清单」表 43 个相对链接全部失效（41 缺失 / 2 状态错位），`check_issue_links` 正则不覆盖 `../<state>/` 形式 → 无门禁拦截

- 领域：文档账本一致性（master 索引 × 引用门禁覆盖盲区；re-init 残留）
- 优先级：P4（纯文档；不影响三账本门禁——门禁本来就不校验这类链接）
- 置信度：高（python 逐链接实测：43/43 失效；两处清单表均可复现）

## 现象

`issue/new/00-index.md` 顶部两处「发现清单」表（`:10` 的「## 发现清单」与 `:378`
附近的逐轮清单）用**相对链接** `../<state>/<name>.md` 指向 issue 文件，例如：

```text
| dead-01 **done** | ... |
| io-02 **done** | ... |
| perf-01 **reviewed** | ... |
| dead-03 | ... |
```

自 `issue/new/` 解析 `../done/` == `issue/done/`。实测该形式的链接：

```text
../state 链接总计 43：ok 0 / 失效 43
  - 41 个目标文件完全不存在（re-init 丢失，见 `ledger-01`）
  - 2 个状态错位：`../done/` 前缀指向 `sem-14-sto51-padding-branch-divergence`
    与 `sem-58-truncate-manifest-shape-empty-and-no-match`（二者实际在 `reviewed/`）
    （两文件实际在 `issue/reviewed/`）
```

`issue/done/` 当前为空，`dead-01`/`io-02`/`struct-01`/`test-01`/`perf-01…06`
等 41 个早期 issue 文件在 `issue/` 下**不存在**（`find issue -name 'dead-*'` 仅命中
本仓新立的 `dead-08`）。

## 根因

1. `ledger-01`（第 29 轮，P1）：issue 仓 re-init 只保留 `issue/{new,done,todo}` 在途
   文件，丢失 `issue/reviewed/`（及 `reject/`）历史。其修复只针对**门禁可见**的
   `issue/<state>/<name>.md` 形式引用（重建 29 个存档桩）——共 1209 处、
   `00-index.md` 顶表那批 `../<state>/` 链接**不在其内**，故未建桩。
2. `scripts/check_issue_links.sh:63` 的正则是
   `issue/(done|new|reviewed|reject)/([A-Za-z0-9][A-Za-z0-9_.\-]*\.md)`——
   **要求字面前缀 `issue/`**；`../done/...`（以及裸名 `dead-03-....md`）**不匹配**，
   被静默跳过。故 `--check` 长期报 `0 missing / 0 wrong-state`，这两张表的死链
   从未被拦截（两处清单表之外的正文引用多为 `issue/<state>/` 形式，由桩托住，绿）。
3. `ledger-01` 无「表内链接」类目；后续轮的「三账本全绿」证据只覆盖门禁可见形式，
   盲区不被视为回归。

## 建议（二选一或并用）

- 文档面：把两处清单表的历史链接改注为**纯文本**或统一成 `issue/<state>/<name>.md`
  形式——但 41 个文件已随 re-init 不可恢复，无桩可指，只能标注
  「历史清单（文件已随 re-init 丢失，见 `issue/reviewed/ledger-01-*.md`）」，
  2 个状态错位的（sem-14 / sem-58）改指 `issue/reviewed/`。
- 门禁面（更彻底）：`check_issue_links.sh` 的 `LINK_RE` 增补 `(?:\.\./|issue/)`
  前缀替代，使 `../<state>/` 相对链接也纳入校验（这样 `doc/` 与 `issue/` 内任何
  状态目录相对链接的漂移都会红）。

## 验收方式

```bash
cd /Users/z/git/db/kv/eloqstore2fkv
python3 - <<'PY'
import re, pathlib
idx = pathlib.Path('issue/new/00-index.md').read_text(encoding='utf-8')
pat = re.compile(r'\]\(\.\./(done|new|reviewed|reject)/([A-Za-z0-9][A-Za-z0-9_.\-]*\.md)\)')
states = ('done','new','reviewed','reject')
bad = [m.group(0) for m in pat.finditer(idx)
       if not any((pathlib.Path('issue')/s/m.group(2)).is_file() for s in states)]
print('broken ../state links:', len(bad))
PY
# 期望：改注后 0（或门禁增补后由 check_issue_links 直接拦截）
bash scripts/check_issue_links.sh --check   # 增补 LINK_RE 后应报出这些失效/错位
```

## 处置记录

- 2026-11-08 第 38 轮结构 / 文档账本域发现（issue 仓；fkv 冻结 worktree
  `/tmp/r29/fkv` HEAD `6d3f11a` 仅作环境取证，本条纯 issue 仓文档面）。
- 备注：与 `doc-16` 无重叠（doc-16 是 COMPAT_BASELINE/doc/*.md 的已删组件叙述，
  本条是 master 索引的历史清单相对链接 × 门禁正则盲区）。

## 实现记录（2026-11-08）

- 修复（issue 仓，doc + 门禁脚本）：
  1. **门禁盲区关闭**：`scripts/check_issue_links.sh` 的 `LINK_RE` 扩展为同时匹配
     `issue/<state>/…` 与相对写法 `../<state>/…`（原正则只认前者，导致早期索引表
     的相对链接长期不被校验）。
  2. **链接修复**：`issue/new/00-index.md` 106 个链接——**11 个修复**为当前实际
     状态目录、**95 个去链保留文本**（目标随 re-init 永久丢失，历史叙述保留）；
     `issue/todo/00-index.md` 同步处理（修复 3 / 去链 13）。
  3. 两处**状态错位**（属 `reviewed/` 却写 `../done/`）纠正：`sem-14` / `sem-58`
     存档桩内摘录的相对链接。
- 验证：`check_issue_links --check` → **1393 引用 / 0 missing / 0 wrong-state**
  （扩展正则后仍全绿，即此前不可见的漂移已清零）；`--fix` 幂等无改动；
  baseline sync 44/44 / 84889 chars；看板与 report 一致。

## 复审记录（2026-11-08，第 39 轮结构域）

- 判定：**达标**，移入 reviewed/。对提交态（`git archive HEAD` 抽出的只读副本）
  计量：`check_issue_links --check` **1394 引用 / 0 missing / 0 wrong-state**、
  `--fix` 幂等无改动；`issue/new/00-index.md` 保留 11 个修正后的
  `../reviewed/` 链接、其余去链为纯文本；`issue/todo/00-index.md` 已无
  `../<state>/` 链接。正则扩展口径正确。
