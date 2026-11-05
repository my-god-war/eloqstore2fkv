# eng-72 KvOption 字段在 struct / Default / INI 表三处重复，RuntimeConfig / ShardConfig 再平行一份

## 位置

- `crate/foundation/fkv_config/src/lib.rs:17-84`：`KvOption` 字段声明（约 68 个字段）
- 同文件 `:86-157`：`impl Default` 再逐字段写一遍默认值
- 同文件 `:244-340`：`load_from_ini` 的 `match key` 再逐字段写一遍字符串键与解析函数
- `crate/engine/fkv_runtime/src/lib.rs:33-50`：`RuntimeConfig` 又重复了 `num_threads/page_size/restart_interval/pages_per_file_shift/segment_size/segments_per_file_shift/auto_reopen_*` 等 14 个字段
- `crate/engine/fkv_shard/src/lib.rs:31-57`：`ShardConfig` 再重复 15 个字段
- `crate/engine/fkv_runtime/src/lib.rs:221-244`：`start_impl` 把 `KvOption` 的字段逐项抄进 `ShardConfig`，默认值也再写一遍（`8 * 1024 * 1024`、`3`、`16`）
- `crate/engine/fkv_store/src/lib.rs:144-177`：`Store::new` 再定义一遍默认值（`pages_per_file_shift: 18`、`manifest_limit: 8 MiB`、`branch_name: "main"`）
- `crate/facade/fkv_sdk/src/lib.rs:24-45`：`KvStore::new` 又手工逐项从 `KvOption` 取值、
  逐个 `with_*` 构造 `Store`（当前已补齐大部分字段，但仍是与 `Runtime::start_with_options`
  平行的第二套映射；行为差异归 eng-58，此处只谈结构）

## 现状

同一个配置字段最多出现 5 次：

| 字段 | struct | Default | INI 表 | Runtime/Shard | Store 默认 |
|---|---|---|---|---|---|
| `segment_size` | `:51` | `:122` | `:292` | `RuntimeConfig:39` / `ShardConfig:37` | `:643` 等 |
| `pages_per_file_shift` | `:74` | `:145` | `:296` | `RuntimeConfig:38` / `ShardConfig:36` | `:161` |
| `manifest_limit` | `:26` | `:97` | `:267` | `ShardConfig:55` | `:164` |
| `overflow_pointers` | `:75` | `:146` | `:297` | `ShardConfig:51` | `:170`（另一常量） |

后果：

1. 新增一个 knob 要在 4~5 个地方同步改；漏一处就是“struct 有字段但 INI 永远读不到”或“INI 能配但 Store 永远用默认值”。
2. 已经存在不对称：`store_path_lut`、`standby_master_store_path_weights`、`standby_master_store_path_lut` 只在 struct 中，INI 表里没有；`data_file_size` 只在 INI 表里，struct 中没有（单独变量）。
3. 默认值分叉风险：`auto_oom_retry_times` 在 `KvOption` 是 5（`:153`），`Runtime` 在缺少 options 时写死 3（`:232`）；`Store::new` 的 18 / 8 MiB 与 `KvOption` 的默认值各写一份，未来调参必然漂移。
4. `RuntimeConfig` / `ShardConfig` 与 `KvOption` 是平行继承关系，不是投影关系；调用方需要同时理解三套字段名和默认值。

## C++ 实现

- `include/kv_options.h`：`KvOption` 一个结构体，字段用 default member initializer 定义唯一默认值。
- `src/kv_options.cpp`：INI 解析按 section 读写同一结构体；键名集中在 `ParseOption` 一处；`ValidateOption` 也针对同一结构体。
- `Shard` / `IouringMgr` 只持 `const KvOption *`，不再定义平行配置结构。

## 建议

1. 用声明宏把“字段 + 默认值 + INI 键 + 解析器”写一次，宏展开出 struct、`Default`、`set_ini_field`：
   ```rust
   kv_options! {
     num_threads: u16 = 1, parse_num;
     buffer_pool_size: u64 = 128 * MB, parse_size;
     // ...
     store_path: Vec<String> = Vec::new(), split_list;
   }
   ```
   `data_file_size` 这类派生键在宏外单独处理即可。
2. 引入一个 `StoreConfig`（page/segment/manifest/compression/overflow/amplify/large_value 字段），由 `impl From<&KvOption>` 生成；`Store::new` 只接收 `StoreConfig`，删除十几个 `with_*` setter 的默认值副本。
3. `RuntimeConfig` 收缩为 runtime 专属字段（`num_threads`、`root`、`queue_size`、归档参数），其余从 `&KvOption` 派生；`ShardConfig` 直接删除或变成 `StoreConfig` 的别名。
4. `KvStore::new` 与 `Runtime::start_with_options` 共用同一个 `StoreConfig::from(options)`，结构上消除两套接线（行为修复仍归 eng-58）。
5. 默认值只允许出现在宏/常量里；`Store::new`、`Runtime` 的 `map_or(...)` 不再写字面量。

## 严重度

P2（结构维护成本与默认值漂移；当前字段值大多一致，尚无新行为分叉）

## 核验记录（2026-11-04）

`KvOption` 三段（struct `:17-84`、Default `:86-157`、INI `:258-324`）实读确认；`RuntimeConfig` / `ShardConfig` 字段与 `start_impl` 的逐项映射实读；`Store::new` 默认值、`KvStore::new` 的逐项 `with_*` 实读。字段不对称（`store_path_lut` 等无 INI 键、`data_file_size` 无 struct 字段）由全段对照确认。与 eng-58（校验/SDK 丢字段）、cfg-01（INI 语义）不重复：本条只谈“定义重复”。

## DONE（2026-11-04）

### 收敛清单

1. `KvOption` 三处字段表合一：`crate/foundation/fkv_config/src/lib.rs` 新增 `kv_options!`
   声明宏，一行写出「字段 + 类型 + 默认值 + INI 节 + 解析器」，单遍展开出
   struct、`Default`、`apply_ini_field`。新增 knob 只需改一行，struct / Default / INI
   三处漂移从结构上不可能再出现。
   - 节语义：`Run` / `Permanent`（各自节生效）、`Ext`（两节均可）、`Skip`（无 INI 键）。
   - 特例保留手写：`store_path` / `standby_master_store_paths` 一次写路径与权重两字段；
     `data_file_size` 仍是派生键（换算 `pages_per_file_shift`）。
   - 字段不对称按原语义保留：权重 / lut 表为 `Skip`，`pages_per_file_shift` 等扩展键仍可配。
2. 默认值集中：新增 `DEFAULT_AUTO_OOM_RETRY_TIMES = 5` 供 runtime 的
   `options=None` 回退共用（原来是写死 `3`，与 `KvOption` 的 5 分叉）；
   `DEFAULT_MAX_PROCESSING_TIME_US` 上移到 `fkv_types`，删除 `fkv_config` /
   `fkv_shard` 两份同名常量；runtime 的 `8 * 1024 * 1024` 改用
   `fkv_types::DEFAULT_MANIFEST_LIMIT`。`Store::new` 的默认值已全部引用
   `fkv_types` 常量（`DEFAULT_PAGES_PER_FILE_SHIFT` / `DEFAULT_MANIFEST_LIMIT` /
   `MAIN_BRANCH_NAME`），无需再改。
3. `KvOption → ShardConfig` 单一投影：`Runtime::start_impl` 把 per-shard 结构体
   提到循环外只解一次（`shard_id` / `fd_limit` 循环内覆盖，其余 `..clone()`），
   回退默认值只出现一次，不再逐 shard 重复 `map_or`。
4. 测试：`config.rs` 增 `ini_empty_keeps_defaults_and_ignores_derived_keys`，
   锁定「空配置等于 Default」与「派生键无 INI 键」。

### 验证

- `cargo check -p fkv_config -p fkv_runtime -p fkv_shard -p fkv_sdk` 通过
- `cargo nextest run -p fkv_config -p fkv_runtime -p fkv_shard -p fkv_sdk -p fkv_store --no-fail-fast`
  180 passed / 1 skipped
- `cargo clippy` 四 crate `--no-deps --all-features --all-targets -- -D warnings` 零警告
  （顺手把 `fkv_shard::write_gate` 两个仅测试使用的方法标 `#[cfg(test)]` 消掉 dead_code）

### 遗留（拆出，未在本条做）

- `fkv_sdk::KvStore::new` 仍是第二套 `KvOption → Store` 逐项 `with_*` 接线；
  彻底合并需要 `StoreConfig`（issue 建议 2），会改 `fkv_store` 公开构造与行为边界，
  归 eng-58 一起做。
- `KvOption.overflow_pointers` 默认值 16 与 `fkv_page::DEFAULT_OVERFLOW_POINTERS`
  仍是两个 crate 各自定义；合并需给 `fkv_page` 加 `fkv_types` 依赖，价值低于改动面。

## 复审（2026-11-05 r6b）

通过。

- `kv_options!` 声明宏落地：`crate/foundation/fkv_config/src/lib.rs:123`（macro_rules）、`:164`（字段表单遍展开）、`:145 apply_ini_field`、`:281`（load_from_ini 走生成键表）；`store_path`/`standby_master_store_paths` 特例手写在 `:265` 注释处。
- 默认值集中：`:55 DEFAULT_AUTO_OOM_RETRY_TIMES=5`；`fkv_types/src/lib.rs:37 DEFAULT_MAX_PROCESSING_TIME_US=400`、`:32 DEFAULT_MANIFEST_LIMIT=8MiB`（fkv_config/fkv_shard 无同名重复常量）。
- 后续新增 knob 走宏单行（如 eng-34b 的 `yield_budget_us` 在 `:259` 即一行定义），验证了「三处漂移结构性消除」的声称。
