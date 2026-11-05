# sto-63 「L1 = Local + append + 无压缩」基线宣称：证据缺失且三要素均有失真

## 位置
- Rust: `COMPAT_BASELINE:1-2`（`eloqstore main 82b774c... / 兼容等级：L1（Local + append + 无压缩）`）、`doc/migration.md:22-23、166`（oracle 空、V2–V6 未系统执行）、`fixtures/cpp/`、`fixtures/rust/`（空目录，无夹具）、`crate/engine/fkv_compaction/src/lib.rs:1`、`crate/foundation/fkv_config/src/lib.rs:76`（data_append_mode 无消费）
- C++: `include/kv_options.h:394-397`（`data_append_mode` 默认 false）、`src/replayer.cpp:319-391`（append/pooled 双分配器）、`doc/migration.md:211`（§8.1 压缩保留条件差异，自身已承认非逐字节）

## 现状
基线宣称三要素逐项核对：

1. **「append」名不副实**：Rust 引擎并非「支持 append 模式」，而是**无条件恒 append**——`data_append_mode` 字段存在但无消费点（sto-59），replay 恒建 `AppendAllocator`（sto-52）。C++ 默认 `data_append_mode=false`（PooledFilePages）。因此「Rust ↔ C++ 双向可读」目前只可能验证过 append 子模式（如果验证过的话——见第 4 点），而 C++ 默认模式的互操作未声明也未验证。
2. **「无压缩」是默认值事实而非能力边界**：`enable_compression` 默认 false（`fkv_config:148`），与 C++ 默认一致；但压缩功能已实现（D58：zstd standalone+字典、字典随 manifest 持久化、读路径解压），且 `doc/migration.md:211` 自己记录了压缩保留条件差异（「varint 前缀 + 压缩体」比较 vs C++ 仅比较压缩体，1~5 字节边界决策可能不同）。即：压缩路径**逻辑等价可读，但非逐字节决策等价**，宣称里没有说明。
3. **「Local」互操作无证据**：`doc/migration.md:22-23` 明写 C++ oracle 仅有占位、44 个 `_cpp` crate 未实现；`migration.md:166`「V2–V6 还没有系统化执行」；`fixtures/cpp`、`fixtures/rust` 为空目录。即没有任何 V2 builder 差分、V4 交叉读（C++ 写 → Rust 读/续写 → C++ 读）证据支撑「字节兼容」。
4. **分支/云模式宣称不可验证**：branch manifest framing（`BranchManifestMetadata.file_ranges`、segment mapping delta 的 Fixed32 长度前缀+varint 对）、archive 命名（`manifest_<branch>_<term>_<tag>`）、文件命名（`data_<id>_<branch>_<term>`）在代码阅读上与 C++ `replayer.cpp:208-231、file_gc.cpp:583-658、common.h:477-498` 一致，但同样零 V2/V4 证据。另外 sto-51（损坏尾部不覆盖）、sto-52（分配器重建缺失）表明即便格式兼容，恢复语义也不等价。

## C++ 实现
对照对象即基线 commit `82b774c`（HEAD 确认一致）。C++ 侧对「兼容」的口径在 `doc/migration.md` 之外无机器可验证产物（oracle 未建）。

## 差异与风险
- **宣称过度**：L1 暗示「Local 已字节兼容」，实际是「未验证 + 仅 append 路径 + 恢复语义不等价」；验收标准 5（V4 交叉读）在文档里被列为未完成项，与 `COMPAT_BASELINE` 的 L1 声明自相矛盾；
- **账本失真**：`fkv_ledger` 的 `compat` 字段默认 L1（`tool/fkv_ledger/src/baseline.rs:26`），所有 unit 都会被自动归到「L1 兼容」档，真实缺口（sto-51/52/56 等）被基线声明掩盖；
- **风险路径**：使用者按 L1 声明做互操作（C++ 写默认模式 store → Rust 续写）时，分配器策略切换（sto-52/59）与撕裂尾部缺陷（sto-51）都未在基线声明中排除。

## 对齐建议
- 基线声明改为如实口径，例如：`L1-partial：Rust 仅 append 分配模式；C++ 互操作限定 data_append_mode=true + enable_compression=false 的 store；字节兼容待 V2/V4 证据`；
- 在 `COMPAT_BASELINE` 同文件登记排除项：PooledFilePages 行为、压缩保留决策边界、撕裂尾恢复语义（sto-51 修复前）；
- 把 sto-51 修复 + `cpp/eloqstore` oracle + fixtures 生成器列为 L1 升级到「已验证」的前置条件（`doc/migration.md` §5 的 V0–V4 逐级）；
- `fkv_ledger` 的 compat 字段区分「声明」与「验证」两列，默认不再自动填 L1。

## 性能优化建议
无。

## 严重度
P1（行为不一致/功能缺失——基线与证据脱节）

## 审查结论（2026-11-04）

### 判定

**成立，保留并拆分到 `issue/todo/`。** 核心指控（L1 宣称缺少 V2/V4 证据、仅覆盖 append 子模式、压缩非逐字节等价）在当前源码上仍成立。原文部分定位与「账本失真」一条已过时，需要按本轮核验修正。

### 逐条核验

| 原陈述 | 现状 | 修正 |
|---|---|---|
| `COMPAT_BASELINE:1-2` 宣称 L1（Local + append + 无压缩） | **成立** | 文件仍为两行 L1，未附范围/排除项 |
| `doc/migration.md:22-23` oracle 空、44 个 `_cpp` 未实现 | **成立** | 第 24 行仍写「仅有占位」，`fixtures/cpp` 为空 |
| `doc/migration.md:166` V2–V6 未系统执行 | **成立** | 原文一致 |
| `fixtures/cpp`、`fixtures/rust` 空目录、无夹具 | **部分过时** | 目录仍空，但 `fixtures/README.md` 与 `fkv_ledger fixture/verify` 已落地清单流水线；缺的是真实 payload 与 oracle 二进制。该缺口已由 `issue/todo/01-cpp-oracle-and-fixtures.md` 跟踪，不再重复立项 |
| `crate/engine/fkv_compaction/src/lib.rs:1` 空壳 | **过时** | 该 crate 不存在；compaction 实现在 `crate/engine/fkv_task_compact/src/lib.rs`（30 行，逐页搬页）。空壳/差异由 `sto-56` 跟踪 |
| `fkv_config:76` `data_append_mode` 无消费 | **部分过时** | 字段现位于 `fkv_config/src/lib.rs:101`，默认 false（173 行），standby/cloud 下自动置 true（275、295 行）；但引擎仍无任何 `AllocMode` 分支，恒 `AppendAllocator`（`fkv_store/src/lib.rs` 多处 `AllocMode::Append`）。「无消费点」应表述为「引擎未按该选项选择分配器」 |
| 压缩已实现、保留条件与 C++ 不同 | **成立** | `fkv_store` 有 `with_compression_level`，`doc/migration.md` §8.1 记录了 varint 前缀比较差异 |
| `fkv_ledger` 默认把所有 unit 填 L1 | **不成立** | `compat TEXT` 仅建表（`db.rs:19`）与扫描占位（`scan.rs:164`），无自动填值；`baseline.rs:26` 的 `unwrap_or("L1")` 只是读取基线文件失败时的等级兜底。无需为此改账本结构 |
| sto-51/52 恢复语义不等价 | **成立** | 由独立 issue 跟踪 |

### 范围收敛

本轮只修**文档口径**，不伪造未验证结论，不改引擎行为：

1. `COMPAT_BASELINE` 等级降为 `L1-partial`，并登记范围与排除项。
2. `doc/migration.md` 状态表与验证阶梯如实反映「零 V2/V4 证据」；§8.1 标题去掉「不影响双向互读」的过度断言。
3. 分配器、压缩、恢复语义、oracle/fixtures 的前置条件交叉引用 sto-51/52/56/59 与 `01-cpp-oracle-and-fixtures`。

## DONE（2026-11-04）

拆分并落地为 `sto-63a`、`sto-63b`，两项均已 `DONE` 并归档 `issue/done/`。

## 复审（2026-11-05 r6）

**通过（父条目，拆分闭合）。** 子项核实：sto-63a（COMPAT_BASELINE L1-partial + 排除块，本次复核 `COMPAT_BASELINE:2-12` 实含四项排除与升级前置）、sto-63b（doc/migration.md :24/:168/:221「L1-partial」口径与 §8.2 排除表）均落地。账本 `baseline.rs` 保留说明块的修复见 63a 复核。无剩余开发量。
