# sto-63a 基线等级改为如实口径并登记排除项

## 位置

- `COMPAT_BASELINE:1-2`
- `tool/fkv_ledger/src/baseline.rs:57-64`（`write` 只重写前两行）

## 方案

1. 第二行等级由 `L1（Local + append + 无压缩）` 改为
   `L1-partial（Local + Rust 仅 append 分配 + 无压缩；C++ 交叉读写待 V2/V4 证据）`。
2. 首两行后追加固定说明块，逐项登记：
   - 分配模式：Rust 恒 `AppendAllocator`；C++ 默认 `data_append_mode=false`
     （`PooledFilePages`）不在对齐范围，互操作验收限定 append store（sto-59）。
   - 压缩：验收默认 `enable_compression=false`；压缩保留决策非逐字节等价
     （varint 前缀 + 压缩体 vs 仅压缩体）。
   - 恢复语义：撕裂尾（sto-51）与分配器重建（sto-52）未等价，不纳入互操作保证。
   - 证据：`fixtures/cpp` / `fixtures/rust` 无 payload，oracle 未实现，V2/V4 未执行。
3. 修 `baseline::write`：重写首两行时保留其后的说明块，避免 `fkv_ledger sync`
   擦除排除项。

## 验收

- `cat COMPAT_BASELINE` 含 `L1-partial` 与四项排除说明。
- `baseline::read` 仍能解析 commit/date/level（`兼容等级：` 前缀不变）。
- `cargo check -p fkv_ledger` 通过。

## DONE（2026-11-04）

- 落地：`COMPAT_BASELINE` 改为 `L1-partial` 并登记分配模式 / 压缩 / 恢复语义 / 证据四项排除；
  `doc/migration.md` 状态表、§5、§8.1、§8.2 同步如实口径。
- `tool/fkv_ledger/src/baseline.rs`：`write` 保留首两行后的说明块，`sync` 不再擦除排除项。
- 验证：`cargo test -p fkv_ledger --test baseline` 1 passed；
  `cargo check --workspace --all-targets --all-features` 通过；`cargo fmt -p fkv_ledger -- --check` 通过。
- 遗留：C++ oracle、真实 fixtures、V2/V4 交叉读仍未完成，由 `01-cpp-oracle-and-fixtures` 与
  sto-51/52/56/59 跟踪；本轮不伪造兼容结论。

## 复审（2026-11-05 r6）

**通过。** 核实：`COMPAT_BASELINE` 首两行为 commit 行 + 「兼容等级：L1-partial（…待 V2/V4 证据）」，其后固定说明块含分配模式/压缩/恢复语义/证据四项排除及升级前置（sto-51/oracle/fixtures）；`tool/fkv_ledger/src/baseline.rs` `write` 保留前两行之后的块（doc 注释与实现以当前源码为准，本次实读 COMPAT_BASELINE 全文确认块未被 sync 擦除）。
