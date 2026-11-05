# sto-57c 未迁移 KvOption 字段决策登记

## 位置
- `crate/foundation/fkv_config/src/lib.rs` 模块文档
- C++：`include/kv_options.h:37,210,222,445`

## 方案
在 crate 模块文档登记：
- `comparator_`：不迁移，fkv 盘面只支持默认字节序比较器（fmt-20）。
- `global_registered_memories` / `pinned_memory_chunks`：不迁移，io_uring 固定 buffer 注册，fkv IO 走 compio。
- `partition_filter`：随 sto-61 的 `DsShardId` 路由落地，当前加字段即死代码。
- `store_path_lut`：字段保留对齐 C++，构建时机在 store_path 接入 runtime（eng-72）。
- 默认值 fallback 语义：不采纳 C++ 静默 fallback，保持 `InvalidArgs`。

## 验收
- 文档可检索到四个字段的处置结论；无新增死字段。

## DONE（2026-11-04）

- `fkv_config` 模块文档新增「未迁移字段（决策登记）」：`comparator_`、`global_registered_memories`、
  `pinned_memory_chunks` 标注不迁移；`partition_filter` 指向 sto-61；`store_path_lut` 指向 eng-72；
  同时写明 INI 默认值 fallback 保持 `InvalidArgs` 的有意差异。
- 未新增字段，无死代码。

## 复审（2026-11-05 r6b）

通过。决策登记在盘：`crate/foundation/fkv_config/src/lib.rs:5-11` 模块文档「未迁移字段（决策登记）」——`comparator_` / `global_registered_memories` / `pinned_memory_chunks` 标不迁移、`partition_filter` 指向 sto-61；fallback 保持 `InvalidArgs` 的有意差异同处写明（`:14` data_append_mode 语义登记）。`kv_options!` 宏字段表中无对应死字段。
