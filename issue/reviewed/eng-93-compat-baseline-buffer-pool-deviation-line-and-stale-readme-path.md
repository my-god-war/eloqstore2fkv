# eng-93 P3：COMPAT_BASELINE 欠 buffer pool 偏离登记行；fixtures/README.md 的 eng-91 链接已过期

来源：r7 第 2 组复审（`issue/reviewed/00-r7-group2.md`）。两项收尾欠账，均无人跟踪，
按 eng-91 先例（无主清理归档即失联）立项。

## 事实（2026-11-05 r7 亲读）

1. rev-41（已复审）裁定用户态 buffer pool 四项为有意偏离，其 DONE「登记」小节明言
   「**COMPAT_BASELINE 登记行待协调者统一补**（eng-91 刚清理基线文件，本会话不动
   `COMPAT_BASELINE` / `fixtures/` / `tool/fkv_ledger/`）」。r7 复核时 eng-91 已收尾、
   协调阻塞已解除，但 `COMPAT_BASELINE` 全文仍无 buffer pool / swizzle / data page
   cache 条目（全文 21 行亲读，:4-20 无相关行）。`doc/migration.md` §8.1（:248-251）
   已有四行完整登记，COMPAT_BASELINE 侧缺口只剩这一行。
2. `fixtures/README.md:154` 写的是
   `issue/done/eng-91-compat-baseline-evidence-line-stale-and-baked-into-fixtures.md`；
   r7 复审通过后 eng-91 已 mv 入 `issue/reviewed/`，该链接成为过期路径——恰是
   eng-91 自己消灭过的那类失效引用，复审会话按约束无权改 `fixtures/`，只能在此接力。

## 修法

1. `COMPAT_BASELINE` 说明块（`fkv_ledger sync` 保留区）追加一行，建议口径对齐
   `doc/migration.md` §8.1：用户态 buffer pool / 页级 pin / WaitingZone / swizzle /
   用户态 data page cache 不做 1:1——index 侧由 `IndexCache`（Clock+代际+CacheLedger）
   替代，data 侧由内核 page cache（默认 buffered）/ 直读（`with_direct_io`）替代
   （io-21）；`enable_data_page_cache` 为解析不生效的死旋钮。引用
   `issue/reviewed/rev-41-…md`。
2. 加行后**必须**重跑 `scripts/gen_fixtures.sh` 刷新 38 份 `meta.txt` 的 `baseline=`
   （`tool/fkv_ledger/src/fixture.rs:342` 把全文压单行烤进语料，不加行不重跑即失同步
   ——这正是 eng-91 修过的坑）；自检 payload 0 字节差异。
3. `fixtures/README.md:154` 路径改 `issue/reviewed/eng-91-…md`。

## 验收

- `grep -n "buffer pool\|data page cache" COMPAT_BASELINE` 有命中；
- 38/38 `meta.txt` 的 `baseline=` 与新 COMPAT_BASELINE flatten 后逐字节一致；
- `grep -n "issue/done/eng-91" fixtures/README.md` 0 命中。

## 甄别（2026-11-05 r7 立项）

- 去重：全 `issue/` grep `COMPAT_BASELINE` 仅命中 todo 的 rev-42a / rev-44 / eng-90
  （均为引用非承接）与 new 索引；无条目承接登记行或 README 路径。rev-41 / eng-91
  两份 md 的残留段是出处记录，归档后无执行载体。
- P3 文档任务，无代码改动；可与任意 lane 并行，只需独占 COMPAT_BASELINE +
  fixtures 写权限一个短暂窗口。

## 甄别（2026-11-05 r7）

**采纳，`issue/new/` → `issue/todo/`。三项事实指控全部亲验为真；另核实出一处
同类的相邻过期引用（见下第 4 条），建议并入同一轮修，非独立立项。**

### 核实结论（逐条亲验）

1. **COMPAT_BASELINE 缺口属实**：全文 21 行亲读，说明块 :4-20 无任何
   buffer pool / 页级 pin / WaitingZone / swizzle / data page cache 条目；
   `grep -n "buffer pool\|data page cache" COMPAT_BASELINE` 0 命中
   （即本条验收第一项当前不过）。
2. **rev-41 声称 vs 实际**：rev-41 DONE「登记」小节（:157-158）明言
   「**COMPAT_BASELINE 登记行待协调者统一补**（eng-91 刚清理基线文件，本会话
   不动 COMPAT_BASELINE / fixtures/ / tool/fkv_ledger/）」——即 DONE 从未声称
   已加行，是显式欠账转交；r7 复审（rev-41 md :183-185）也已核实未补并转本条。
   `doc/migration.md` §8.1 :248-251 四行（页级 pin / WaitingZone / swizzle /
   data page cache，均标「有意偏离 + rev-41」）确在位，COMPAT_BASELINE 侧只剩
   登记行这一步。
3. **fixtures 当前同步、改基线必失同步**：按 `tool/fkv_ledger/src/fixture.rs`
   `baseline_text()`（:342 附近：逐行 trim、滤空行、`" | "` 连接）复刻 flatten，
   全量比对 38/38 份 `fixtures/**/meta.txt` 的 `baseline=` 与当前 COMPAT_BASELINE
   **逐字节一致**（仅 1 种取值）——现状无失同步，但反证修法第 2 条的必要性：
   只加行不重跑 `gen_fixtures.sh`，38 份将全数变陈旧烤文（eng-91 修过的原坑）。
4. **README:154 属实 + 相邻新发现**：`fixtures/README.md:154` 确写
   `issue/done/eng-91-…md`，该文件今在 `issue/reviewed/`（eng-91 DONE 第 3 条
   按「归档终位 done/」写链接，随后复审把 eng-91 挪入 reviewed/，自己造出过期
   路径）。同类相邻：`COMPAT_BASELINE:13` 与 `fixtures/README.md:157` 的
   `issue/done/02-cpp-create-branch-omits-segment-mapping.md` 同样过期（02 也在
   reviewed/），且 :13 已烤进全部 meta.txt——本轮反正要动 COMPAT_BASELINE 并重跑
   gen_fixtures，一并改指 `issue/reviewed/` 近零成本，避免再留一轮欠账。

### 去重与边界

- 全 `issue/` grep `COMPAT_BASELINE`：todo 仅 rev-42a（自述「RootMetaMgr 偏离
  未声明」）、rev-44（自请登记 GlobalRegisteredMemory 范围外）、eng-90（上下文
  引用）——三者各管各的偏离，无条目承接本登记行；grep `gen_fixtures` /
  `issue/done/eng-91` 在 open 条目（new/todo/reject）中仅本条命中。
- **与 eng-91（reviewed）的边界**：eng-91 管的是**旧证据行**（「无 payload /
  oracle 未实现」失实措辞 + `issue/todo/01-` 旧引用）的改正与 meta 刷新，已收尾；
  本条管的是 **rev-41 新裁定的偏离登记行**（当时 eng-91 刚清理基线，rev-41 主动
  不碰）**及 eng-91 归档后遗留的 done→reviewed 路径失联**。两轮缺口成因、时点
  均不同，非重复立项。

### 开发计划

1. `COMPAT_BASELINE` 说明块（`fkv_ledger sync` 保留区）追加一行，口径对齐
   `doc/migration.md` §8.1：用户态 buffer pool / 页级 pin / WaitingZone /
   swizzle / data page cache 不做 1:1——index 侧由 `IndexCache`（Clock+代际+
   CacheLedger）替代，data 侧由内核 page cache（默认 buffered）/ 直读
   （`with_direct_io`）替代（io-21）；`enable_data_page_cache` 为解析不生效的
   死旋钮（INI 面兼容保留，sto-57）；引用 `issue/reviewed/rev-41-…md`。
   同轮把 :13 的 `issue/done/02-…` 改指 `issue/reviewed/02-…`。
2. 重跑 `scripts/gen_fixtures.sh` 刷新 38 份 `meta.txt` 的 `baseline=`；脚本需
   **构建**（`cpp/oracle/build.sh` 编 C++ oracle + `cargo run -p fkv_ledger` 编
   Rust 工具），`ELOQSTORE_SRC=/Users/z/git/db/kv/eloqstore`（HEAD 须为基线 pin
   `82b774c`，eng-91 先例）——**与并行会话协调**：独占 cargo 构建锁 + COMPAT_
   BASELINE/fixtures 写窗口，避开正动 `tool/fkv_ledger/` 或 `fixtures/` 的 lane。
   自检 payload 0 字节差异（对拍 payload.bin/dict.bin md5）。
3. `fixtures/README.md:154` 改指 `issue/reviewed/eng-91-…md`；同轮 :157 的
   `issue/done/02-…` 一并改 reviewed。

### 验收（在原三条上追加）

- 原 3 条（grep buffer pool 命中；38/38 meta.txt 与新基线 flatten 逐字节一致；
  `issue/done/eng-91` 在 README 0 命中）全部保持；
- 追加：`grep -rn "issue/done/" COMPAT_BASELINE fixtures/README.md` 0 命中。

> 认领（2026-11-05 04:08，本会话子代理开发中）。lane：COMPAT_BASELINE、fixtures/README.md、fixtures/**/meta.txt（gen_fixtures 重跑）、scripts/gen_fixtures.sh。

## DONE（2026-11-05）

三步全做，验收四条全过。

### 要点

1. `COMPAT_BASELINE` 说明块（`fkv_ledger sync` 保留区）追加 buffer pool 偏离
   登记行（现 :14-19，插在 branch+large value 与证据行之间，证据行仍居末），
   口径对齐 `doc/migration.md` §8.1 四行：用户态 buffer pool / 页级 pin /
   WaitingZone（首访 single-flight）/ swizzle 直取快路径 / data page cache
   均不做 1:1（有意偏离，rev-41）——index 侧由 `IndexCache`（Clock+代际+
   CacheLedger）替代，data 侧由内核 page cache（默认 buffered）/ 直读
   （`with_direct_io`）替代（io-21）；`enable_data_page_cache` 为解析不生效
   的死旋钮（INI 面兼容保留，sto-57）；引用
   `issue/reviewed/rev-41-mem-cached-page-buffer-pool-not-implemented.md`。
   同轮 :13 的 `issue/done/02-…` 改指 `issue/reviewed/02-…`。
2. `git -C eloqstore rev-parse HEAD` 确认 82b774c 后重跑
   `ELOQSTORE_SRC=/Users/z/git/db/kv/eloqstore scripts/gen_fixtures.sh`
   （oracle 就 /tmp/fkv_oracle 重建，exit 0；双向校验 cpp 16 pass / 6 skipped、
   rust 16 pass / 0 fail）刷新 38 份 `meta.txt`。
3. `fixtures/README.md:154`（eng-91）与 :157（02）改指 `issue/reviewed/`。

### 验收证据

- `grep -n "buffer pool\|data page cache" COMPAT_BASELINE` → :14、:15 命中。
- 38/38 `meta.txt` 的 `baseline=` 与新 COMPAT_BASELINE flatten（逐行 trim、
  滤空、`" | "` 连接）逐字节一致，`distinct_values=1`、`mismatch=0`。
- `grep -n "issue/done/eng-91" fixtures/README.md` → 0 命中。
- `grep -rn "issue/done/" COMPAT_BASELINE fixtures/README.md` → 0 命中。
- payload 对拍：重跑前备份 40 份二进制 md5（38 `payload.bin` + 2 `dict.bin`，
  /tmp/fixtures-binary-md5-before.txt），重跑后 diff 0 差异。
- `cargo nextest run -p fkv_ledger` → 22 passed / 0 skipped / 0 failed。

### 先例记录：登记行曾被并行写入覆盖丢失

约 40 分钟前协调者曾直接在 `COMPAT_BASELINE` 加过一行「页缓存：…（rev-41）」，
03:41 被并行会话整文件写入覆盖丢失（回退 20 行、无 buffer pool）。教训：
**基线行加完必须立即烤进语料**——本次加行后随即重跑 `gen_fixtures.sh`，登记行
  已进 38 份 `meta.txt` 的 `baseline=`，再覆盖需付出全量语料重生成代价，持久化
  成立。后续任何会话改 `COMPAT_BASELINE` 都应同轮重跑 `gen_fixtures.sh`（eng-91
  原坑，`tool/fkv_ledger/src/fixture.rs:342` flatten 机制）。

## 复审（2026-11-05 r7）

**通过。** 重点核对项（38 份 meta.txt 与 COMPAT_BASELINE 逐字节一致）实测：

- `COMPAT_BASELINE` 全文 26 行亲读：buffer pool 偏离登记行在 :14-19（页级 pin /
  WaitingZone 首访 single-flight / swizzle 直取 / data page cache 不做 1:1，
  IndexCache / 内核 page cache + `with_direct_io`（io-21）替代、
  `enable_data_page_cache` 死旋钮 sto-57、引用
  `issue/reviewed/rev-41-mem-cached-page-buffer-pool-not-implemented.md` :19），
  与 `doc/migration.md` §8.1 口径一致；:13 已改指 `issue/reviewed/02-…`；
  证据行仍居末（:20-26）。
- **flatten 逐字节比对**：按 `tool/fkv_ledger/src/fixture.rs:342-354`（`lines`
  →`trim`→滤空→`" | "` 连接）复刻，38/38 份 `fixtures/**/meta.txt` 的 `baseline=`
  与新 COMPAT_BASELINE 逐字节一致：`distinct_values=1`、`mismatch=0`（1503 字节）。
  上轮复审登记的「行加完必须立即烤进语料」已兑现。
- `fixtures/README.md:154` → `issue/reviewed/eng-91-…md`、:157 →
  `issue/reviewed/02-…md` 亲读确认；`grep "issue/done/" COMPAT_BASELINE
  fixtures/README.md` 0 命中。验收四条全过（buffer pool grep 实际 :14/:15/:18
  三处命中，DONE 列举 :14/:15 为不完全列举，非失实）。
