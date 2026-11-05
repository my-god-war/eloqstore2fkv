# fmt-18b KvOption → SDK/Shard/Runtime 透传 index restart 间隔

## 位置
- `crate/foundation/fkv_config/src/lib.rs:48-49,130-131,362-367`（两字段定义 / 默认 / 解析）
- `crate/facade/fkv_sdk/src/lib.rs:38`（只传 `data_page_restart_interval`，index 侧丢失）
- `crate/engine/fkv_shard/src/lib.rs:86,287,362`（`ShardConfig.restart_interval` 单值）
- `crate/engine/fkv_runtime/src/lib.rs:41,322`（`RuntimeConfig.restart_interval` 单值；有 `KvOption` 时仍只按它构造）

## 方案
1. `KvOption::validate`：两 interval 均须非 0（页构建器本来就拒绝 0，提前到配置层报 `InvalidArgs`）。
2. `KvStore::new`：`Store::new(.., options.data_page_restart_interval).with_index_restart_interval(options.index_page_restart_interval)`。
3. `ShardConfig` 增 `index_restart_interval: u16`；`Shard::start` 用它配置 store。
4. `Runtime::start_impl`：给了 `KvOption` 时 data/index 各取对应旋钮，否则都回落到 `RuntimeConfig.restart_interval`（不新增 RuntimeConfig 字段，避免改动 15 个测试构造点；`RuntimeConfig.restart_interval` 语义 = 无 KvOption 时的共用回退值）。
5. 不新造平行配置结构：字段沿既有 KvOption → ShardConfig → Store builder 链传递。

## 验收
- `fkv_config` 测试：两旋钮可独立解析，0 值 validate 拒绝。
- `fkv_runtime` 测试：`start_with_options` 用 `index_page_restart_interval` 与 `data_page_restart_interval` 不等时两页布局各自生效（可用 `index_cache`/页字节断言或沿用 fmt-18a 的字节测试思路）。
- `cargo nextest run -p fkv_config -p fkv_shard -p fkv_runtime -p fkv_sdk` 全绿；clippy 零警告。

## DONE（2026-11-04）

改动文件：
- `crate/foundation/fkv_config/src/lib.rs`：两字段补注释；`validate` 拒绝任一 restart 间隔为 0（页构建器本就失败，提前到配置层）。
- `crate/facade/fkv_sdk/src/lib.rs`：`Store::new(data 值)` + `.with_index_restart_interval(index 值)`，index 旋钮不再丢失。
- `crate/engine/fkv_shard/src/lib.rs`：`ShardConfig` 增 `index_restart_interval`（`restart_interval` 保留为 data 侧），`Shard::start` 透传到 store builder；8 个 shard 测试配置补字段。
- `crate/engine/fkv_runtime/src/lib.rs`：给了 `KvOption` 时 data/index 各取对应旋钮，否则回落到 `RuntimeConfig.restart_interval`（不新增 RuntimeConfig 字段，15 个测试构造点不动）。

新增测试：
- `fkv_config/tests/config.rs`：`ini_restart_intervals_are_independent`（两旋钮独立解析）+ `invalid_options` 补 0 值拒绝。
- `fkv_runtime/tests/options.rs`：`restart_intervals_reach_shard_store`（data=8/index=2 写入 200 条，scan / reopen / get 全绿）。

## 复审（2026-11-05 r6）

通过。链条：`fkv_config/src/lib.rs:182`（Ext 键）+ `:311-312`（0 值拒绝）→ `fkv_sdk/src/lib.rs:44` → `fkv_shard/src/lib.rs:131,540` → `fkv_runtime/src/lib.rs:1142-1143`。测试 `fkv_config/tests/config.rs:145 ini_restart_intervals_are_independent`、`fkv_runtime/tests/options.rs:147 restart_intervals_reach_shard_store` 均在位。
