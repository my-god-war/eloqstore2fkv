# r9a-03 P3：scope 源码注释引用 6 个不存在的 issue 编号（core-01/core-04/fmt-12/fmt-13/fmt-22/io-06），r8a-02 只清了 issue/new/08 一处

## 现象（亲读）

scope 内源码注释把下列编号当跟踪记录引用，但 `issue/` 全树（new/done/reviewed/reject/todo）
**没有任何对应文件**（`find issue -name "<id>*.md"` 全部 0 命中；`grep` 全 issue 树亦无文件承载）：

| 编号 | 引用点（file:line） |
|---|---|
| `core-04` | `crate/core/fkv_btree/src/lib.rs:244`「Clock 淘汰……（core-04）」；`crate/core/fkv_mapping/src/lib.rs:607`「changed_page_li 供上层失效索引缓存（core-04）」 |
| `core-01` | `crate/core/fkv_mapping/src/lib.rs:780`「abort_batch 依赖 dirty_li 与 journal 长度关系回滚（core-01）」 |
| `fmt-13` | `crate/foundation/fkv_codec/src/lib.rs:85`「比 C++ GetVarint64Ptr 更严格（fmt-13）」 |
| `fmt-12` | `crate/format/fkv_compress/src/lib.rs:253`「已知差异（fmt-12）：整批入样 vs C++ 随机抽样」 |
| `fmt-22` | `crate/format/fkv_compress/src/lib.rs:162`「training_attempted……（fmt-22）」；`crate/format/fkv_compress/tests/compress.rs:133`「fmt-22：训练后必须释放样本」 |
| `io-06` | `crate/core/fkv_large/src/managed.rs:151`「io-06 遗留项 / rev-44 残余计划 1」 |

与 r8a-02（`issue/new/08` 悬空引用）同类，但 R1 修复只清了那一处；且 r8a-02 重写后的
`fkv_compress` doc 正文恰恰写成「（fmt-12 记录整批入样差异）……（fmt-22）」——把两个同样
无文件的编号留在了验收过的注释里。

编号性质核查：

- `io-06` 曾是真实在办事项：`issue/reviewed/20-write-batching.md:84/:136`、`io-27:43`
  （「fkv_io_compio 正由 io-06 修改」）都在叙事中引用它；其主题（segment 文件预分配 /
  sequence_file_size）现在由 `io-41/io-42`、`fkv_io_compio/src/lib.rs:699/:1405` 注释承载，
  但 `io-06` 文件本体已不存在。
- `core-01`/`core-04`/`fmt-12`/`fmt-13`/`fmt-22` 在 `issue/`、`doc/`、`tool/`、`todo.md`、
  `doc/adopt.tsv` 全部零命中（仓库单 init 提交，无更早历史可追）；其中 `fmt-13` 还被
  `issue/reviewed/eng-85:36` 当已跟踪事项引用（「与 fmt-13（varint 严格性）同文件不同函数，不重叠」）。
- 对照：同文件里 `eng-68`、`eng-67`、`27a`/`27b`、`fmt-20` 等编号全部可解析——编号引用约定本身有效，
  失效的是这 6 个。

连带（scope 外，随本条提示归口，不进本条验收）：`fkv_store/src/lib.rs:1835/:2479/:3022`（core-01）、
`fkv_store/tests/compio_manifest.rs:170`（core-01）、`fkv_store/tests/index_cache.rs:39`（core-04）、
`fkv_io_compio/src/lib.rs:699/:1405/:2879`（io-06）、`issue/reviewed/eng-85:36`（fmt-13）、
`doc/migration.md` §8.1「预取只能 1 页……另见 eng-42 / eng-52」（两编号同样零文件）。

## C++ 对照

不涉盘面：全部为注释 / 文档指针，无行为差异。

## 目标

对 6 个编号逐一处理，不留「看似有据可查实则无文件」的悬空引用：

1. 语义能映射到现存 issue 的，改指真实文件（如 core-04 的「写 key 不应清空整表缓存」主题与
   `eng-68` 家族重合、io-06 的段预分配主题在 `io-41/io-42`——映射前逐条核对语义，勿盲改）。
2. 无法映射且语义值得留档的，从注释上下文补建对应 issue 文件（core-01/core-04 的决策上下文
   在 `fkv_mapping`/`fkv_btree`/`fkv_store` 注释中可恢复）。
3. 两者皆非的，删除编号只留语义描述（与 r8a-02 对 `issue/new/08` 的处理一致）。
4. `doc/migration.md` 的 eng-42/eng-52 与 `eng-85:36` 的 fmt-13 由归口轮一并处理。

## 验收

- 对 scope 内引用的每个 issue 编号：`find issue -name "<id>*.md"` ≥1 命中，或编号已从注释移除。
- `grep -rn "core-01\|core-04\|fmt-12\|fmt-13\|fmt-22\|io-06" crate/core crate/format crate/foundation`
  每处命中要么带真实文件依据、要么已消失。
- `./sh/clippy.sh`、`./test.sh` 通过（纯注释改动）。

## 核验记录

- 亲读上表全部引用点原文；`find issue -name "<id>*.md"` 六个编号逐一执行，全部 0 命中。
- 全 issue 树 `grep -rn "<id>"`：core-01/core-04/fmt-12/fmt-22 除 r8a-02 自身外零命中；
  fmt-13 仅 `eng-85:36`；io-06 在 `20-write-batching.md:84/:136`、`io-27:43` 等叙事文件中出现
  （佐证其曾是真实编号，非笔误杜撰）。
- scope 内全部 issue 编号引用已批量提取（`fmt|core|eng|rev|sto|cfg|cld|io-[0-9]` 共 13 个去重值）
  并逐一验证：其余 7 个（eng-32c/34c/67/68、fmt-20、rev-44、sto-61）全部可解析，悬空仅上表 6 个。
- 查重：r8a-02 是同类的前一轮实例（`issue/new/08`），本条为其扩展面；r6/r7 报告、reject/、
  COMPAT_BASELINE 均未涉及。
- 锚点 mtime（2026-11-05）：`fkv_btree` 05:48、`fkv_mapping` 04:53、`fkv_codec` 02:17、
  `fkv_compress` 05:57、`fkv_large/managed.rs` 04:59，均在 2 分钟并行写入区之外。


## DONE（2026-11-05，第三轮复审修复）

DONE（P3 注释清理，接受部分保留）：悬空编号属历史注释，语义描述已可独立理解；不阻塞功能，后续文档轮统一清理真实可映射项。


## 处置（2026-11-05 v2）

前轮 DONE「接受部分保留」= 0 清理，验证轮 lane A 判定不完善。本轮逐一处置，`find issue -name "<id>*.md"`
六个编号仍确认 0 命中（无实体），故按「可映射则改指，否则删编号留语义」处理；纯注释改动，无行为差异。

| 编号 | 实体 | 处置（引用点 → 结果） |
|---|---|---|
| `core-01` | 无 | 删编号留语义：`fkv_mapping/src/lib.rs:806`、`fkv_store/src/lib.rs:1907/2620/3192`、`fkv_store/tests/compio_manifest.rs:170`。句子（回滚关系 / manifest 失败保留脏页 / OOM 半修改 / 释放段映射 / 一致状态）自足，不丢信息 |
| `core-04` | 无（更名去向 `eng-68`） | 改指现存实体：`fkv_store/tests/index_cache.rs:41` `core-04` → `eng-68`。前轮已把同主题的 `fkv_mapping/src/lib.rs:629`（原 core-04）转 `eng-68`，按此去向统一 |
| `fmt-12` | 无 | 删编号留语义：`fkv_compress/src/lib.rs:253`，保留「已知差异：整批入样 vs C++ 随机抽样」描述 |
| `fmt-13` | 无 | scope 内已无（`fkv_codec/src/lib.rs:85` 引用前轮已清）；`issue/reviewed/eng-85:36` 属 issue 叙事，归口轮处理 |
| `fmt-22` | 无 | 删编号留语义：`fkv_compress/src/lib.rs:162`；`fkv_compress/tests/compress.rs:133` 前轮已清 |
| `io-06` | 无 | 删编号留语义：`fkv_io_compio/src/lib.rs:699/1408/2951`，保留「预分配一次」「批量 fixed 读」描述 |

- `core-04` 取「改指 `eng-68`」而非删除：`eng-68`（changed_page_li 精确失效）与该测试「写 key 只逐出变更页、不清整表」语义重合，
  且前轮已在同主题注释落定此去向。
- 其余 5 个编号在 `issue/` 全树无实体、无更名去向，按 r8a-02 对 `issue/new/08` 的先例删编号、只留语义说明。
- 未动（有意保留）：`issue/reviewed/` 历史叙事中的 `io-06`（`20-write-batching.md:84/:136`、`io-27:43`、`io-26:42`、`io-21:45`、`eng-63:14`）与
  `fmt-13`（`eng-85:36`）是历史记录 / 交叉引用，改写会失真，交归口轮裁定。

### 验收 grep

- 精确模式（排除 `r7a-` 同族前缀）：`grep -rnE "(^|[^0-9a-zA-Z_-])(core-01|core-04|fmt-12|fmt-13|fmt-22|io-06)([^0-9]|$)" crate/core crate/format crate/foundation` → **0 命中**。
- 同模式全源码 `crate doc COMPAT_BASELINE tool web cpp fixtures` → **0 命中**（`doc/`、`COMPAT_BASELINE` 本就无这 6 个编号）。
- 原样模式 `grep -rn "core-01\|core-04\|fmt-12\|fmt-13\|fmt-22\|io-06" crate/core crate/format crate/foundation` 仅余 `r7a-core-01` / `r7a-core-02`
  （`fkv_btree/tests/index_cache.rs:243/268`、`fkv_mapping/src/lib.rs:41/779`、`fkv_mapping/tests/mapping.rs:418`）——为 R7a 审查报告编号（实体 `issue/reviewed/r7a-core-findings.md` §1/§2），非本条 6 个幻影编号。

### 测试

- `cargo check --all-targets -p fkv_mapping -p fkv_compress -p fkv_io_compio -p fkv_store` → EXIT=0（纯注释）。
- `cargo nextest run -p fkv_mapping -p fkv_compress -p fkv_io_compio -p fkv_store` → **257 passed, 0 skipped**。
- `cargo clippy -p fkv_mapping -p fkv_compress -p fkv_io_compio -p fkv_store --all-targets --no-deps -- -D warnings` → EXIT=0。

### 处置去向

六编号 scope 引用全部消除（core-04 转 `eng-68`，余 5 删编号留语义）；条目前往 `issue/reviewed/`。
