# cld-53 P2：`fkv_sdk::KvStore::new` 无视运行模式，Cloud/Standby 配置静默降级纯本地盘

来源：第 5 轮 runtime/cloud lane 侦查，协调者亲验（Rust 侧）。

## 证据（2026-11-05 核验）

`crate/facade/fkv_sdk/src/lib.rs:23-48`：`new` 只做 `options.validate()` 后一律
`CompioStore::new(root, ...)` + `with_branch(MAIN_BRANCH_NAME, 0)`；全函数无
`derive_mode()`、无 `cloud_store_path` / `enable_local_standby` 分支。

而 `fkv_runtime::backend_of`（`fkv_runtime/src/lib.rs:712-751` 一带）对同样字段
显式拒绝错误组合——同一配置面两套判定，SDK 侧是「静默按 Local」。

C++ SDK runtime（`eloqstore/src/sdk_runtime/manager.cpp:723-725`）构造完整
`EloqStore`，options 的模式全部生效。

## 问题

SDK 用户配了云路径/standby 期望对应模式，实际只写本地盘且零报错——比报错更糟。
`validate()` 通过不代表模式被尊重。

## 目标

`KvStore::new` 至少对 `derive_mode() != Local` 的配置显式 `Err`
（对齐 `backend_of` 的拒绝口径），错误信息指引用 `fkv_runtime` 入口；
或在 facade 决策（见 eng-84）落地时让 SDK 直接构造 Runtime 后端。
禁止维持静默降级。

## 验收

- 测试：`cloud_store_path` 非空时 `KvStore::new` 返回错误而非本地 store；
  纯 Local 配置行为不变。
- `cargo nextest run -p fkv_sdk`。

## 依赖 / 重叠

- 与 eng-84（Runtime 入口接线）同源，可先落「显式拒绝」再等入口决策；
  sto-58（derive_mode 优先级，done）已定判定规则。

## 核验记录

- 本人实读：`fkv_sdk/src/lib.rs:1-60`。
- 子代理摘录（未复核）：`manager.cpp:723-725`、`backend_of` 行号。

## DONE（2026-11-04）

按 issue 的「至少显式 Err」方案落地（`crate/facade/fkv_sdk/src/lib.rs`）：

- `KvStore::new` 在 `validate()` 之后加模式判定：
  `if options.derive_mode() != StoreMode::Local { return Err(KvError::InvalidArgs) }`。
  注释写明 SDK 只装配本地 `CompioStore`，Cloud / Standby 要走
  `fkv_runtime::Runtime::start_with_options`；并点明 `validate()` 通过不代表模式被尊重。
- 新增测试 `fkv_sdk::sdk::sdk_rejects_non_local_mode`：
  - `cloud_store_path` 非空（`derive_mode() == Cloud`）→ `InvalidArgs`；
  - `enable_local_standby = true` + `standby_master_addr = "local"` →
    `InvalidArgs`（standby 本身已在 12b 接进 Runtime，SDK 仍不装配）；
  - 纯 Local 配置行为不变（`KvStore::new(...).is_ok()`）。
- 影响面：`fkv_cli` 也走 `KvStore::new`，现在把云配置显式报错（不再静默写本地盘），
  `fkv_cli::cli` 测试仍绿。

验证：
- `cargo nextest run -p fkv_sdk` 7 passed；
- `cargo nextest run -p fkv_cli` 1 passed；
- `cargo clippy -p fkv_sdk -p fkv_cli --all-targets --no-deps -- -D warnings` 零警告。

未做（另一支路，归 eng-84 / 12c）：
让 SDK 直接构造 `Runtime` 后端（即 Cloud/Standby 在 SDK 层真正可用）。本条只消除
「静默降级」，入口接线仍按 eng-84 的决策走。

## 复审（2026-11-05 r6）

**通过。** 核实：`fkv_sdk/src/lib.rs:29-31` `KvStore::new` 在 `validate()` 后显式 `derive_mode() != StoreMode::Local → Err(InvalidArgs)`，注释写明 Cloud/Standby 走 `fkv_runtime`。测试 `tests/sdk.rs:11` `sdk_rejects_non_local_mode` 在位。SDK 真正构造 Runtime 后端仍归 eng-84（todo），与 md 记档一致。
