# doc-16 缓存重构删除 `IndexCache` / `IndexNode` / `WritePageCache` / `fkv_task_read` 单飞后，issue 仓文档（COMPAT_BASELINE / migration / roadmap / progress / ledger-classification）仍把已删组件当活跃描述

- 领域：文档账本一致性（并行会话 `f6f08c8 refactor(cache)` 遗留，sem-71 的账本侧孪生）
- 优先级：P3（纯文档；但 COMPAT_BASELINE 是差分对拍的权威口径，误导后续读者/差分器）
- 置信度：高（逐条 grep + 文件存在性实测）

## 现象

`f6f08c8`（主体已并入 `init` `ff63590`）把 `IndexCache` / `IndexNode` /
`CacheLedger` / `WritePageCache`（`Store::with_write_cache_bytes`）收敛为单一
读写共用 `PageCache`（`fkv_store/src/page_cache.rs`），并删除
`fkv_task_read` 的 `IndexRead` / `PageFlight`（`src/flight.rs`）及其
`tests/single_flight.rs`。第 36 轮 sem-71（`4abc60c`）只清理了 fkv 仓 22 个
`.rs` 文件的注释漂移，其验收 grep 明确限定 `-- '*.rs'`；**issue 仓的文档账本
（COMPAT_BASELINE / doc/*.md）未随之更新**，仍以现在时描述已删组件。

## 证据（HEAD `6d3f11a`，冻结 worktree `/tmp/r29/fkv`）

已删源码/测试文件实测不存在：

```text
$ ls crate/engine/fkv_task_read/src/           # → lib.rs  page_buf.rs  （无 flight.rs）
$ ls crate/engine/fkv_task_read/tests/         # → read.rs  read_pool.rs（无 single_flight.rs）
$ ls crate/engine/fkv_store/src/write_cache.rs   # → No such file
$ ls crate/engine/fkv_store/tests/write_cache.rs # → No such file
$ git grep -nE 'LoadGuard|LoadStep|IndexRead|PageFlight|PageCache::retain_mapped'  -- '*.rs'   # → 0
```

issue 仓文档中的过时引用（`/Users/z/git/db/kv/eloqstore2fkv`）：

| 文件:行 | 过时内容 |
|---|---|
| `COMPAT_BASELINE:61` | 「index 侧由 `IndexCache`（Clock+代际+`CacheLedger`）替代」——三类型全删 |
| `COMPAT_BASELINE:62-63` | 「data 写路径…由 `WritePageCache`（`Store::with_write_cache_bytes` 门控…`fkv_store/src/write_cache.rs`）替代」——类型/API/文件全无（现为统一 `PageCache`） |
| `COMPAT_BASELINE:65` | 「`fkv_store/tests/write_cache.rs` 差分锁定」——该测试文件已并入 `tests/page_cache.rs` |
| `COMPAT_BASELINE:1262-1274` | l2-12 条目：`LoadGuard::drop` 广播 `TryAgain`（`fkv_task_read/src/flight.rs:178-192`、等待者 `lib.rs:348-358` 的 `LoadStep::Waiter`、锁定测试 `fkv_task_read/tests/single_flight.rs` 的 `owner_cancel_broadcasts_try_again_to_all_waiters`）——文件/类型/测试全部不存在，且 `lib.rs:348-358` 现为无关的 `read_floor_entry` |
| `doc/migration.md:304` | 引用 `IndexCache::get` 返回的 `&IndexNode`、`fkv_task_read::descend_step` |
| `doc/migration.md:305` | 引用 `RefCell<IndexCache>`、`IndexCache` 冷缓存 miss 论证 |
| `doc/migration.md:306` | 引用 `IndexCache::retain_mapped` / `IndexNode` |
| `doc/roadmap-full-alignment.md:75` | M1 论证「`IndexCache` 无在飞去重」 |
| `doc/roadmap-full-alignment.md:143` | 「Rust 由 `IndexCache`（Clock + 代际 + `CacheLedger`）承担」 |
| `doc/progress.md:95` | rev-41 条目「缓存命中走 IndexCache」（由 `fkv_ledger report` 生成） |
| `doc/ledger-classification.tsv:542` | 同上分类条目（progress.md:95 的来源） |

（注：`doc/ledger-classification.tsv:366/621` 的 `fkv_shard/src/read/flight.rs`
**有效**——该文件存在，属在飞读任务记账，与本次删除的 `fkv_task_read/src/flight.rs`
非同一文件，勿一并改。）

## 影响

- COMPAT_BASELINE l2-12 是一条**「有意差异（登记）」**：它告诉差分器「不得以
  『取消后等待者仍拿到页』要求 fkv」。该机制（读路径首访 single-flight + owner
  取消广播）已随重构删除，条目现描述不存在的代码，后续读者按 `flight.rs:178-192`
  定位会落空（与 sem-71 记录的风险同类）。
- 第 23 轮前后 index 的锚点表把 `l2-12（LoadGuard::drop flight.rs:196）`、
  `l2-13（IndexRead::none lib.rs:234）` 记为「文档说、代码做全部成立」；重构后
  这两条锚点已不成立（`lib.rs:234` 现为 `ReadEntry` 结构体）。
- 三账本门禁**不覆盖**此类漂移：`check_baseline_sync.sh` 只核对
  `baseline=` 压行与 progress 看板计数，不校验 COMPAT_BASELINE 正文的文件/符号。
  故无任何门禁会拦住它。

## 建议（文档面，零代码改动）

按现结构改写上述文档条目：`IndexCache` / `IndexNode` / `CacheLedger` /
`WritePageCache` → 统一 `PageCache`（`fkv_store/src/page_cache.rs`，读写共用）；
l2-12 与 migration.md 304-306 / roadmap 75/143 中「页级 pin / swizzle / 首访
single-flight」条目按「读路径已无首访在飞去重、页读取直接经 `PageCache` 命中或
真读」重述（或标注为已随 `f6f08c8` 失效的历史记录）；`doc/progress.md:95` 与
`doc/ledger-classification.tsv:542` 的分类理由同步后重跑 `fkv_ledger report` 重生成。

## 验收方式

```bash
cd /Users/z/git/db/kv/eloqstore2fkv
grep -nE 'IndexCache|IndexNode|CacheLedger|WritePageCache|with_write_cache_bytes|fkv_task_read/src/flight\.rs|fkv_task_read/tests/single_flight\.rs|LoadGuard|LoadStep' COMPAT_BASELINE doc/*.md doc/*.tsv
# 期望：生产/现时条目零命中（或全部带「已随 f6f08c8 失效」历史标记）
bash scripts/check_baseline_sync.sh --check     # 仍 44/44
bash scripts/check_progress_board.sh --check    # 重生成后仍绿
```

## 处置记录

- 2026-11-08 第 37 轮结构 / 测试 / 死代码 / 文档账本域发现（fkv HEAD `6d3f11a`，
  冻结 worktree）；与第 36 轮 sem-71（fkv `.rs` 注释侧）同根因，账本侧未被其覆盖。

## 实现记录（2026-11-08，coordinator 直修）

- 修复（issue 仓，无 fkv 代码改动）：
  - `COMPAT_BASELINE`「范围与排除项」的 buffer pool 条目重写为**单一 `PageCache`**
    现状（物理页键 / Clock+引用位 / index 与 data 同池 / `tests/page_cache.rs`
    差分锁定），删除 `IndexCache`/`WritePageCache`/`write_cache.rs` 引用；
  - `COMPAT_BASELINE` 的 **l2-12 条目重写**：机制（页级 single-flight 的
    `LoadGuard` 取消广播 / `LoadStep::Waiter`）已随 index 逻辑页缓存删除而移除，
    条目改为「两侧都不存在该状态」的登记 + 新重开条件（fkv 重新引入页级
    single-flight 时须重建取消传播语义与锁定测试）；
  - `doc/migration.md` 三行（页级 pin / WaitingZone 首访 / swizzle 直取）改为
    `PageRef::Shared` / `PageCache` 现状；`doc/roadmap-full-alignment.md` 两处
    （M1 / 排除清单）同步；`doc/ledger-classification.tsv:542` 的来源注记同步。
- 验证：`check_baseline_sync --check` 44/44（`--fix` 刷新压行后）、
  `check_issue_links --check` 1377/0/0、`check_progress_board --check` 绿；
  残留 grep：`IndexCache|WritePageCache|CacheLedger` 在 COMPAT_BASELINE /
  migration / roadmap / tsv **仅剩 1 处历史性提及**（l2-12 新条目里对已删机制的
  说明，属有意保留的历史叙述）。
- **已接受残留（记录在案）**：`doc/progress.md:95`（rev-41 unit 的账本注记）
  仍含 `IndexCache` 字样——该注记文本存于 ledger DB（`.data/motedb.mote`），
  `fkv_ledger classify` 重放**不刷新注记**（实测 `unchanged 543`），
  非本仓文档面；人为改写 DB 注记不属本轮范围，且该行属**历史裁定记录**
  （2026-11-05 分类时的描述），不影响当前对拍口径。

## 复审记录（2026-11-08，第 38 轮结构 / 文档账本一致性域）

- **结论：达标 → 归档 reviewed/**（issue 仓，fkv 冻结 worktree `/tmp/r29/fkv`
  HEAD `6d3f11a` 取证；issue 仓工具未变）。
- 逐条核对修复落地：`COMPAT_BASELINE:60-61`（buffer pool → 单一 `PageCache`）、
  `:1262-1263`（l2-12 重写为「机制已随页缓存重构移除」+ 重开条件）、
  `doc/migration.md:304-307`（页级 pin / WaitingZone 首访 / swizzle 直取 / data page
  cache 改「有意偏离」现状）、`doc/roadmap-full-alignment.md:75`（M1「`PageCache`
  无在飞去重」）/`:143`（「Rust 由单一 `PageCache` 承担」）、
  `doc/ledger-classification.tsv:542`（来源注记 → 「缓存命中走单一 PageCache」）。
- **残留扫描（权威口径）**：全 issue 仓 grep
  `IndexCache|IndexNode|CacheLedger|WritePageCache|write_cache\.rs|LoadGuard|single_flight|PageFlight|IndexRead`，
  命中逐条判定：
  - `COMPAT_BASELINE:1262` / `doc/progress.md:6`（== base line）/ `fixtures/{cpp,rust}/**/meta.txt:4`
    （44 份 baseline= 压行，与 COMPAT_BASELINE 同源）：`LoadGuard`/`LoadStep` 均出自
    l2-12 **重写后**的「已随重构移除」历史叙述 → **允许**（已标注移除）。
  - `doc/roadmap-full-alignment.md:75` (`single_flight`)、`doc/ledger-classification.tsv:243,245`
    (`single_flight`)：指 **fkv_cloud 云下载层的 `single_flight`**（现存、可复用），
    非被删的 `fkv_task_read` 单飞 → **允许**（同名不同物）。
  - `issue/{new/00-index.md, reviewed/sem-71-*, reviewed/struct-47-*}`、
    本文件：历史 issue 记录 / 索引聚合 → **允许**。
  - `doc/progress.md:95` (`IndexCache`)：唯一**无历史标记的活跃式引用** =
    本文件的「已接受残留」（DB 注记，非 fkv 文档面）→ 接受（缘由见上）。
  - `doc/ledger-classification.tsv:542` 已不含被删符号。**其余零命中**。
- 三账本复跑全绿：`check_baseline_sync --check` 44/44（83847 chars）、
  `check_progress_board --check` 看板 + report diff 一致、`check_issue_links --check`
  1377 refs / 0 missing / 0 wrong-state。
- 备注：committed `tsv:542`（「单一 PageCache」）与 committed `progress.md:95`
  （「IndexCache」）对同一 unit 的原因文本不一致——源于 `progress.md` 由**未入库**的
  `.data/motedb.mote` 生成、`classify` 幂等不回刷新注记；`check_progress_board` 只对
  本地 DB 整文件 diff、CI 上跳过 → 属生成物固有口径，不另立案。
