
## 复审（2026-11-04，第三轮）

**已在源码中落地（由其它会话实现），核对无误后关闭。**

逐条核对（`crate/foundation/fkv_config/src/lib.rs`）：

| 待办项 | 状态 | 证据 |
|---|---|---|
| `derive_mode` 改 standby 优先 | 已实现 | `:191-200`：`match self.enable_local_standby` 为唯一真源；`true` 分支按 `standby_master_addr` 分 Master/Replica，`false` 分支按 `cloud_store_path` 分 Local/Cloud。与 C++ `include/eloq_store.h:122-134` `DeriveStoreMode` 顺序一致。 |
| `validate` 清 standby 行为不变 | 已保持 | `:252-258`：`cloud_store_path` 非空时清 `enable_local_standby` 等字段，与 C++ `eloq_store.cpp:208-217` 一致；先 validate 再 derive 仍得 `Cloud`。 |
| 双开关矩阵测试 | 已补 | `tests/config.rs::derive_mode_priority_matrix`：6 组组合 `(enable_local_standby, standby_master_addr, cloud_store_path)` 全覆盖，双开关置位得 `StandbyMaster/StandbyReplica`；另断言 validate 后收敛到 `Cloud`。 |
| `fkv_standby::from_options` 双开关场景 | 已补 | `crate/io/fkv_standby/tests/standby.rs::role_from_mode_and_config_validation`：`enable_local_standby + cloud_store_path` 同时置位、未 validate，断言 role 为 `Replica`（不再 `InvalidArgs`）。 |

验收：

- `cargo nextest run -p fkv_config -p fkv_standby` → **27 passed / 0 failed**（含上述两条）。
- `cargo clippy -p fkv_config -p fkv_standby --no-deps --all-features --all-targets -- -D warnings` → **0 警告**。

无需再改动，本 issue 关闭。

## 复审（2026-11-05 r6）

**通过。** 当前源码复核（行号更新）：

- `fkv_config/src/lib.rs:282-289` `derive_mode`：`enable_local_standby` 为唯一真源（true → 按 `standby_master_addr` 分 Master/Replica），false 才按 `cloud_store_path` 分 Local/Cloud —— standby 优先与 C++ `DeriveStoreMode` 一致；doc 注释（:278-281）同时写明「先 validate 再 derive 仍得 Cloud」的口径。
- 测试 `tests/config.rs:70` `derive_mode_priority_matrix`、`fkv_standby/tests/standby.rs:335` `role_from_mode_and_config_validation` 均在位。
