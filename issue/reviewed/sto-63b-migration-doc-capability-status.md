# sto-63b migration.md 如实描述 L1 能力与证据状态

## 位置

- `doc/migration.md:24`（当前状态表 oracle 行）
- `doc/migration.md:148-166`（验证阶梯）
- `doc/migration.md:207-215`（§8.1 已知差异）

## 方案

1. 状态表新增「兼容基线口径」行，指向 `COMPAT_BASELINE` 的 `L1-partial`
   与排除项；oracle 行补充「无任何真实 payload / 差分结果」。
2. §5 末尾补充：当前 `L1-partial` 仅代表 V0 + 少量 V1；V2/V4 缺证据，
   不得据此声称双向字节兼容。
3. §8.1 标题去掉「不影响双向互读」，改为「代码推断差异（待 V2/V4 验证）」。
4. 新增 §8.2「L1-partial 范围与排除项」表：append/pooled、压缩保留决策、
   sto-51/52 恢复语义、oracle/fixtures 证据缺口，并交叉引用
   `issue/todo/01-cpp-oracle-and-fixtures.md`、`sto-59`、`sto-56`。

## 验收

- 文档中引用的文件/符号真实存在：`fkv_task_compact`、`PageMapper`、
  `AllocMode::Append`、`fixtures/README.md`、`tool/fkv_ledger`。
- 不出现「已验证字节兼容」「双向可读已达成」等无证据断言。
- `cargo check --workspace --all-targets --all-features` 通过。

## DONE（2026-11-04）

- 落地：`COMPAT_BASELINE` 改为 `L1-partial` 并登记分配模式 / 压缩 / 恢复语义 / 证据四项排除；
  `doc/migration.md` 状态表、§5、§8.1、§8.2 同步如实口径。
- `tool/fkv_ledger/src/baseline.rs`：`write` 保留首两行后的说明块，`sync` 不再擦除排除项。
- 验证：`cargo test -p fkv_ledger --test baseline` 1 passed；
  `cargo check --workspace --all-targets --all-features` 通过；`cargo fmt -p fkv_ledger -- --check` 通过。
- 遗留：C++ oracle、真实 fixtures、V2/V4 交叉读仍未完成，由 `01-cpp-oracle-and-fixtures` 与
  sto-51/52/56/59 跟踪；本轮不伪造兼容结论。

## 复审（2026-11-05 r6）

**通过。** 核实：`doc/migration.md:24` 状态表「兼容基线口径」行指向 COMPAT_BASELINE；:168 「L1-partial 只表示代码路径对齐…」；:221 `## 8.2 L1-partial 范围与排除项`。§8.1 标题已无「不影响双向互读」断言（grep 未命中）。文档不出现「已验证字节兼容」类宣称。
