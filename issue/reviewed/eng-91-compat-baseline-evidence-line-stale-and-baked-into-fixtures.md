# eng-91 P3：COMPAT_BASELINE 证据行与事实相反，且已被压进全部 fixture 语料的 meta.txt

来源：r6b 收尾 lane 复核 `issue/reviewed/01-cpp-oracle-and-fixtures.md`（原 done/）时发现。
01 的 md 两次声明「COMPAT_BASELINE 的证据行不在本条 lane」（第二、四轮进展小节），
但全 `issue/` 无任何条目承接该清理，01 归档后此事实将无人认领。

## 证据（2026-11-05 核验）

`COMPAT_BASELINE:10` 仍写：

```
- 证据：fixtures/cpp、fixtures/rust 无 payload；C++ oracle 未实现；V2/V4 未执行。
```

与当前事实相反：

- `fixtures/cpp/` 22 个 case：16 个有真实 oracle 字节 payload（page 7 / index 2 /
  compress 2 / manifest 4 / overflow 1）+ 6 个 mapping/store 占位（0 字节、skipped）；
- `fixtures/rust/` 16 个反向语料 payload；
- `cpp/oracle` 已实现（`cpp/oracle/src/main.cpp`，5 类 16 case 双向 verify 16/16 PASS，
  见 01 的 DONE）；
- `COMPAT_BASELINE:12` 还引用已不存在的路径 `issue/todo/01-cpp-oracle-and-fixtures.md`
  （该文件现经复核入 `issue/reviewed/`）。

更糟的是过期文案被固化进语料本身：`fkv_ledger fixture` 把 `COMPAT_BASELINE` 全文压成
单行写进每个 case 的 `meta.txt` `baseline=` 字段（如
`fixtures/cpp/mapping/empty/meta.txt`，`fixtures/cpp` + `fixtures/rust` 共 38 份），
其中「无 payload；C++ oracle 未实现」字样随语料分发，误导后续逐 case 读者。

## C++ 对照

无 C++ 对照物。这是 fkv 侧「兼容声明与证据账本」的真实性问题：`COMPAT_BASELINE`
是互操作等级（L1-partial）的唯一权威声明，`fkv_bench`/`fkv_ledger` 均读它。

## 建议修法

1. 更新 `COMPAT_BASELINE` 证据块：
   - 「证据」行改为事实：format 层 5 类 16 case C++ 写 → Rust 读（含双向）已在
     `fixtures/{cpp,rust}` 与 CI `fixture-diff` 作业落地；mapping/store 占位 skipped；
     V2/V4（引擎级交叉读写）仍未执行。
   - 「L1 升为已验证前置」行同步改写：oracle + fixtures 双向交叉读已就绪，
     剩余前置为 sto-51（撕裂尾覆写）/ sto-52（GetMapper 重建）修复与 V2/V4 执行。
   - issue 路径改指 `issue/reviewed/01-cpp-oracle-and-fixtures.md`。
2. 重跑 `scripts/gen_fixtures.sh` 刷新全部 `meta.txt` 的 `baseline` 字段，并对
   payload 0 字节差异自检（语料 payload 本身不受影响）。
3. 顺带在 `fixtures/README.md` 「待补」处链接本条，形成闭环。

## 验收

- `grep -n "无 payload" COMPAT_BASELINE` 0 命中；`issue/todo/01-` 引用 0 命中。
- `fixtures/**/meta.txt` 的 `baseline=` 行不再含「C++ oracle 未实现」。
- `scripts/gen_fixtures.sh` 重跑后 `fixtures/cpp` 与已提交语料 payload 仍 0 字节差异。

## 甄别（2026-11-05 r6）

**采纳。三条事实指控全部核实为真，非重复立项，`issue/new/` → `issue/todo/`。**

证据复核（逐条实测）：

- `COMPAT_BASELINE:10` 确写「证据：fixtures/cpp、fixtures/rust 无 payload；C++ oracle
  未实现；V2/V4 未执行。」，与在盘事实相反：
  - `fixtures/cpp` 22 个 case 中 16 个有真实非零 payload（page 7×4096B / index 2×4096B /
    compress 2 / manifest 4 / overflow 1×12288B），仅 mapping 3 + store 3 共 6 个占位
    （0 字节、`status=skipped`、`oracle=-`）；
  - `fixtures/rust` 16 个反向语料全部有真实 payload；
  - oracle 已实现：`cpp/oracle/src/main.cpp` + `standalone.{h,cpp}` + `build.sh` 在盘，
    `issue/reviewed/01-cpp-oracle-and-fixtures.md` 第二批 DONE + r6b 复审通过
    （双向 16/16 PASS、`scripts/gen_fixtures.sh` 可复现 0 字节差异）。
- `:12` 引用 `issue/todo/01-cpp-oracle-and-fixtures.md` 已失效（现为
  `issue/reviewed/`）；同行的 `issue/todo/sto-56`、`sto-59` 同样已迁 `reviewed/`，
  修法一并更正，不只 01 一处。
- 38 份 `meta.txt` 固化属实：`fixtures` 下 38/38 份 `meta.txt` 的 `baseline=` 行同时含
  「无 payload；C++ oracle 未实现」与「issue/todo/01-cpp-oracle」；机制见
  `tool/fkv_ledger/src/fixture.rs:342 baseline_text()`（全文压单行）与 `:127`（写入
  `baseline=`）。
- 去重：全 `issue/` 无其他条目承接此清理。`sto-63a`（reviewed）是该证据行的作者
  （2026-11-04 当时属实，后因 01 落地而过期）；`00-r6-catchup.md:60-65` 与
  `01` 的 r6b 复审小节（:359）均为本条的立项出处记录，非重复。

开发计划（采纳建议修法，两处细化）：

1. 只改 `COMPAT_BASELINE` 首两行之后的说明块：`tool/fkv_ledger/src/baseline.rs:57-75`
   `write` 在 `sync` 时原样保留前两行之后的块，手改证据区不会被擦除；
   「证据」行改为事实（5 类 16 case C++ 写 → Rust 读双向 fixture 已落地，CI
   `fixture-diff` 在跑；mapping/store 占位 skipped；V2/V4 未系统化执行）。
   「L1 升为已验证前置」行同步改写（剩余前置 = sto-51/sto-52 修复 + V2/V4 执行）；
   `:12` 三个 issue 引用全部改指 `issue/reviewed/`。
   措辞注意：按 `doc/migration.md:155-157`，V2=格式差分、V4=交叉读（含续写），
   fixture 证据属格式层双向、不含续写，勿笼统写成「V2/V4（引擎级交叉读写）」，
   以「V2/V4（含续写/引擎级）未系统化执行」为准（`migration.md:167` 口径）。
2. 重跑 `scripts/gen_fixtures.sh` 刷新 38 份 `meta.txt` 的 `baseline=` 字段，
   自检 payload 0 字节差异（语料字节本身不受影响）。
3. `fixtures/README.md:137` 「待补」处链接本条。

## DONE（2026-11-05）

随 manifest snapshot 语料 lane 的首轮改写（其记录的 README CI 流程同步与
abseil pin 说明保留在 `fixtures/README.md`）之后，eng-91 专项收尾按甄别
开发计划三步完成并全量复核：

1. `COMPAT_BASELINE` 证据块改写（只动首两行之后的说明块，`baseline.rs write`
   的保留机制不受影响）：
   - 「证据」行改为事实：format 层 5 类 16 case 双向语料已落地——C++ 写 → Rust
     读（`fixtures/cpp`）、Rust 写 → C++ 读（`fixtures/rust`），CI `fixture-diff`
     重新生成并 `git diff --exit-code fixtures/cpp` 检查漂移；`mapping` / `store`
     占位 skipped；V2/V4（含续写 / 引擎级）未系统化执行——措辞按
     `doc/migration.md:155-157` 与 `:167` 口径，不笼统声称引擎级交叉读写已验证。
   - 「L1 升为已验证前置」行同步改写：剩余前置 = sto-51 / sto-52 修复 + V2/V4
     执行（首轮遗留改动漏 sto-52，本次补齐）。
   - 三个 issue 引用全部改指 `issue/reviewed/`（01、sto-56、sto-59）。
2. `scripts/gen_fixtures.sh` 重跑（`ELOQSTORE_SRC=/Users/z/git/db/kv/eloqstore`，
   其 HEAD `82b774c` 与基线 pin 逐字一致）：38 份 `meta.txt` 的 `baseline=`
   全量刷新，oracle 双向 verify 16/16 PASS。
3. `fixtures/README.md`「待补」新增 V2/V4 条目并链接本条（指向归档终位
   `issue/done/eng-91-…md`，不再制造过期路径）。

验收（issue 三条全过）：

- `grep -n "无 payload" COMPAT_BASELINE` 0 命中；`issue/todo/01-` 引用 0 命中
  （连带 `issue/todo/sto-56`、`issue/todo/sto-59` 亦 0 命中）。
- 38/38 份 `fixtures/**/meta.txt` 的 `baseline=` 行不含「C++ oracle 未实现」，
  且均含新口径「sto-51 / sto-52 修复」。
- 重跑前后 40 个 `payload.bin` / `dict.bin` md5 逐文件一致（0 字节差异），
  语料字节不受影响；差异仅限 38 份 `meta.txt` 的 `baseline=` 字段。
  在盘清点：`fixtures/cpp` 22 case（16 真实 payload + 6 skipped 占位）、
  `fixtures/rust` 16 case 全真实 payload。
- `cargo nextest run -p fkv_ledger`：22/22 PASS，0 失败。
- `cargo clippy -p fkv_ledger --all-features --all-targets -- -D warnings`：
  退出码 0，零警告。
- `tool/fkv_ledger/src/baseline.rs`、`fixture.rs` 无需改动：`baseline_text()`
  压行与 `write()` 保留首两行后块的机制按预期工作。

## 补充（2026-11-05，manifest snapshot 批次）

本批最终把 `COMPAT_BASELINE` 的 L1 前置从「sto-51 / sto-52 修复」收敛为
「sto-51 修复 + V2/V4 执行」（sto-52 已在 `issue/reject/`，不纳入等价），并新增
`branch + large value` 排除项：C++ `CreateBranch` 不写 snapshot 的可选 segment
段，Rust 为保留 large value 继承仍写（见
`issue/new/02-cpp-create-branch-omits-segment-mapping.md`）。
38 份 `meta.txt` 已按最终文本再次刷新；payload 仍 0 字节差异。上文验收段的
「sto-51 / sto-52」措辞以本补充为准。

## 复审（2026-11-05 r7）

**通过。** 逐项实测：

- `COMPAT_BASELINE`（全文 21 行亲读）：`无 payload` / `C++ oracle 未实现` /
  `issue/todo/01-` / `issue/todo/sto-56` / `issue/todo/sto-59` 全部 0 命中；
  证据区 :14-17 为事实口径（5 类 16 case 双向语料 + CI `fixture-diff` +
  `git diff --exit-code` 漂移检查；mapping/store skipped；V2/V4 未系统化执行）；
  L1 前置行 :18-20 = sto-51 修复 + V2/V4 执行，三个 issue 引用均指 `issue/reviewed/`。
- 38 份 `meta.txt` 与当前 `COMPAT_BASELINE` 同步：按 `fixture.rs:342` 的 flatten
  规则（trim + 去空行 + `" | "` 连接）重建全文，与 38/38 份 `baseline=` 逐字节比对，
  **0 份 mismatch**（含 sto-51 新口径）。注：meta.txt mtime 03:39:30 早于
  COMPAT_BASELINE mtime 03:41:21，但内容一致（后者为等值/空白级触碰），无失同步。
- 已知风险核实：rev-41 的页缓存偏离登记行**并未**进入 COMPAT_BASELINE（其 DONE
  明言「登记行待协调者统一补」），故「:14-16 页缓存条目未刷进 meta.txt」的失同步
  不存在——行未加，烤文一致。该欠账已立 `issue/new/eng-93-*.md` 跟踪（加行 +
  重跑 gen_fixtures + 顺带修下条路径）。
- fixtures 清点：cpp 22 meta（16 份非零 payload + 6 skipped 0 字节）、rust 16 meta
  全非零 payload，与 DONE 一致。
- `fixtures/README.md:154` 已链接本条——但指向 `issue/done/eng-91-…md`，本复审
  归档 mv 至 `issue/reviewed/` 后该路径过期；复审会话按约束无权改 `fixtures/`，
  已并入 eng-93 修法第 3 步接力（非本条缺陷，登记不扣分）。
