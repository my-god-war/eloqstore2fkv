# eng-71 同一事实多处定义：页对齐 / 树深 / 文件名 / 时间助手 / 值编码在 2~4 个 crate 各写一份

## 位置

同一语义的常量或函数在多个 crate 重复定义，且类型、命名、默认值已经出现分叉：

| 事实 | 重复定义 |
|---|---|
| 4 KiB 页对齐 | `crate/core/fkv_buf/src/lib.rs:14`（`PAGE_ALIGN: usize`）、`crate/foundation/fkv_config/src/lib.rs:14`（`PAGE_ALIGN: u64`）、`crate/core/fkv_large/src/lib.rs:25`（`SEGMENT_ALIGN: u32`）、`crate/format/fkv_manifest/src/lib.rs:24`（`MANIFEST_ALIGN: usize`） |
| 树深上限 64 | `crate/engine/fkv_task_read/src/lib.rs:18`、`crate/engine/fkv_task_scan/src/lib.rs:20`、`crate/engine/fkv_task_write/src/lib.rs:33`（三处 `MAX_TREE_DEPTH`） |
| `.tmp` 后缀 | `crate/foundation/fkv_types/src/lib.rs:24`（`TMP_SUFFIX`）、`crate/io/fkv_io_compio/src/term.rs:17`（同名私有常量）；字面量 `crate/io/fkv_io_compio/src/lib.rs:1123,1518,1583`、`crate/io/fkv_standby/src/lib.rs:229,243` |
| data / segment / manifest 文件名前缀 | `crate/foundation/fkv_types/src/lib.rs:20-25` 已有 `FILE_NAME_*`；字面量仍散落在 `crate/io/fkv_io_compio/src/lib.rs:441-442,489,1583`、`crate/io/fkv_standby/src/lib.rs:30-32`（另起 `DATA_PREFIX`/`SEGMENT_PREFIX`/`MANIFEST_PREFIX`）、`crate/engine/fkv_cloud/src/lib.rs:991`、`crate/foundation/fkv_types/src/lib.rs:181,188` 自身解析函数 |
| active manifest 文件名 | `crate/io/fkv_io_compio/src/lib.rs:1123,1127` 硬编码 `"manifest_main_0"` / `"manifest_main_0.tmp"`；`fkv_types::manifest_file_name` 已有现成实现 |
| 文件名拼装 | `crate/foundation/fkv_types/src/lib.rs:145-158` 的 `data_file_name` 与 `segment_file_name` 除前缀外逐字节相同 |
| mapping 值编码（低 3 位 ValType + 高位 id） | `crate/core/fkv_mapping/src/lib.rs:12-60`（`TYPE_BITS`/`encode_*`/`INVALID_VALUE`）与 `crate/format/fkv_manifest/src/lib.rs:28-60`（`VAL_TYPE_BITS`/`VAL_*`/`encode_*`/`invalid_mapping_value`）各一份 |
| segment 默认布局 | `crate/io/fkv_io/src/lib.rs:128-129`（256 KiB / shift 7）、`crate/foundation/fkv_config/src/lib.rs:122,125`（字面量）、`crate/engine/fkv_store/src/lib.rs:643,1153,1642`（`map_or(7, ...)`） |
| `pages_per_file_shift` 默认 18 | `crate/engine/fkv_store/src/lib.rs:161`、`crate/io/fkv_io_mem/src/lib.rs:35`、`crate/foundation/fkv_config/src/lib.rs:145` |
| manifest 上限 8 MiB | `crate/engine/fkv_store/src/lib.rs:164`、`crate/engine/fkv_runtime/src/lib.rs:243`、`crate/foundation/fkv_config/src/lib.rs:97` |
| 分支名 `"main"` | `crate/engine/fkv_store/src/lib.rs:155`、`crate/io/fkv_io_compio/src/lib.rs:503`、`crate/engine/fkv_cloud/src/lib.rs:30`、`crate/core/fkv_root/src/lib.rs:50`、`crate/engine/fkv_recovery/src/lib.rs:33`、`crate/engine/fkv_shard/src/lib.rs:284,393`、`crate/facade/fkv_sdk/src/lib.rs:36`；`fkv_types::MAIN_BRANCH_NAME` 已有 |
| 毫秒墙钟 | `crate/engine/fkv_store/src/lib.rs:48-53`、`crate/engine/fkv_shard/src/lib.rs:309-313`、`crate/engine/fkv_runtime/src/lib.rs:180-186`、`crate/facade/fkv_sdk/src/lib.rs:226-231` 四份 |
| “无页”哨兵 | 全仓 39 处 `PageId::MAX`（root / ttl_root / next / prev / empty 各自语义）；`fkv_mapping::FREE_PAGE_NONE` 只覆盖空闲链 |

## 现状

- 这些副本当前数值恰好一致，所以没有立即的行为分叉；但每处都是独立的“事实来源”，任何一处被改（例如 64 KiB 页环境把 `MANIFEST_ALIGN` 调大）都会让格式层、恢复层、standby 层各走一套值。
- 已经能看到分叉迹象：`manifest_main_0` 与 `manifest_file_name(MAIN_BRANCH_NAME, 0)` 是两条独立生成路径；`term.rs` 的 `TMP_SUFFIX` 与 `fkv_types::TMP_SUFFIX` 是两份；`PAGE_ALIGN` 在三个 crate 里是三种整数类型。
- 最危险的是 mapping 值编码：`fkv_mapping` 写盘前编码、`fkv_manifest` 序列化 snapshot 时再编码/解码；两份 `(id << 3) | tag` 只要有一处漂移，盘面会静默写坏，且单元测试各自通过。
- 四个 `now_ms` 都用 `SystemTime::now()` + `unwrap_or(0)`；一旦要换 `coarsetime` 或统一时钟注入，需要改四处，容易漏。

## C++ 实现

C++ 是单一定义：
- `include/common.h:356-357`（`ManifestFileName(term)`）、`:501`（`BranchManifestFileName(branch, term)`）；`data_file_name` / `segment_file_name` 同样只有一套拼装函数。
- `include/types.h` 定义 `MaxPageId` / `MaxFilePageId` 等哨兵；`include/kv_options.h` 是全部默认值的唯一来源。
- `MappingSnapshot::Serialize`（`src/storage/page_mapper.cpp:600-620`）直接复用 `EncodeFilePageId`，不存在第二套值编码。

## 建议

1. 在 `fkv_types` 建 `layout` 模块，作为唯一来源：
   ```rust
   pub const PAGE_ALIGN: usize = 4096;
   pub const PAGE_ALIGN_U64: u64 = PAGE_ALIGN as u64;
   pub const MAX_TREE_DEPTH: usize = 64;
   pub const NULL_PAGE: PageId = PageId::MAX;
   ```
   其余 crate 改为 `use fkv_types::{PAGE_ALIGN, MAX_TREE_DEPTH, NULL_PAGE}`；`fkv_buf` 增加对 `fkv_types` 的依赖（foundation 在下、core 在上，方向正确）。
2. 文件名：`parse_data_file_name` / `scan_dir` / `fkv_standby` 全部改用 `FILE_NAME_*`；`data_file_name` 与 `segment_file_name` 收敛为一个 `file_name(prefix, file_id, branch, term)`；`manifest_path` 改 `manifest_file_name(MAIN_BRANCH_NAME, 0)`，tmp 路径追加 `TMP_SUFFIX`。
3. 值编码：把 `ValType`、`encode_file_page_id`、`encode_page_id`、`decode_id`、`INVALID_VALUE` 移到 `fkv_types`（或 `fkv_codec`），`fkv_mapping` 与 `fkv_manifest` 只 re-export/调用一份。
4. segment 布局：`fkv_io` 的 `DEFAULT_SEGMENT_SIZE` / `DEFAULT_SEGMENTS_PER_FILE_SHIFT` 作为配置默认值来源；`Store` 统一用 `SegmentLayout::segments_per_file_shift()`，删除 `map_or(7, ...)`。
5. 时间：`fkv_types::now_ms()` 一份，内部实现可替换（coarsetime / 注入时钟）；四个调用点改为该函数。
6. 哨兵语义：新增 `NULL_PAGE`（root/next/prev 空值）与 `FREE_PAGE_NONE`（空闲链尾）两个名字，替换裸 `PageId::MAX`，让“最大合法页号”与“空值”不再混用。

## 严重度

P2（当前无行为分叉；值编码与 manifest 对齐两处一旦漂移即盘面损坏，建议在 oracle 差分前收敛）

## 核验记录（2026-11-04）

上述行号均按当前源码实读/grep 确认：四份 `PAGE_ALIGN`、三份 `MAX_TREE_DEPTH`、两份 `TMP_SUFFIX`、两份值编码、四份 `now_ms`、`manifest_main_0` 字面量、segment 默认值与 `map_or(7, ...)` 三处、`"main"` 八处。`fkv_types::MAIN_BRANCH_NAME` 与 `FILE_NAME_*` 已存在但未被这些点使用，属实测。

## DONE（2026-11-04）

### 判定与收敛

- 页对齐：`fkv_types::PAGE_ALIGN`（usize）+ `PAGE_ALIGN_U64` 为唯一来源。删除 `fkv_buf::PAGE_ALIGN`、`fkv_config::PAGE_ALIGN`、`fkv_large::SEGMENT_ALIGN`；`fkv_manifest::MANIFEST_ALIGN` 保留为 `fkv_types::PAGE_ALIGN` 的别名（`fkv_standby` 仍在导入，待其并行迁移结束后删除）。
- 树深：`fkv_types::MAX_TREE_DEPTH`；删除 three task crates 的三份定义。
- `.tmp`：`fkv_io_compio::term` 与 `tmp_path`、manifest 目录扫描统一用 `fkv_types::TMP_SUFFIX`。
- 文件名：`data_file_name` / `segment_file_name` 收敛到私有 `id_file_name`；解析函数改用 `FILE_NAME_*`；compio 的 `data_` / `segment_` / `manifest_` 前缀检查改用常量；`manifest_path` 改 `manifest_file_name(MAIN_BRANCH_NAME, 0)`。
- 分支名：`fkv_store` / `fkv_io_compio` / `fkv_cloud` / `fkv_recovery` / `fkv_sdk` 改用 `MAIN_BRANCH_NAME`。
- 值编码：`ValType` / `VAL_TYPE_BITS` / `encode_file_page_id` / `encode_page_id` / `decode_id` / `is_file_page_id` / `INVALID_VALUE` 唯一实现移到 `fkv_types`；`fkv_mapping` 与 `fkv_manifest` 删除副本，调用点直接引用 `fkv_types`。
- 段布局默认值：`DEFAULT_SEGMENT_SIZE` / `DEFAULT_SEGMENTS_PER_FILE_SHIFT` / `DEFAULT_PAGES_PER_FILE_SHIFT` / `DEFAULT_MANIFEST_LIMIT` 移到 `fkv_types`（`fkv_config` 不能依赖 `fkv_io`，避免反向依赖）；Store 两处 `map_or(7, ...)` 与 `MemStore` 默认 18 全部改用常量。
- 时间：`fkv_types::now_ms()` / `now_us()` 内部用 `coarsetime`，作为唯一入口；`fkv_store::now_ms`、`fkv_sdk::now_ts`、`fkv_api::archive_tag` 改为调用它。
- 无页哨兵：新增 `fkv_types::NULL_PAGE`，`task_read` / `task_scan` / `task_write` / `store` / `recovery` 生产路径替换裸 `PageId::MAX`；`fkv_mapping::FREE_PAGE_NONE` 移到 `fkv_types`。

### 保留（并行会话占用，未硬改）

- `fkv_shard::wall_clock_ms` 与 `"main"` 字面量。
- `fkv_runtime` 的 `manifest_limit: 8 * 1024 * 1024`。
- `fkv_standby` 的 `DATA_PREFIX` / `SEGMENT_PREFIX` / `MANIFEST_PREFIX` 与 `manifest_*.tmp` 拼装。
- 上述文件迁移完成后，删除 `fkv_manifest::MANIFEST_ALIGN` 别名并改 `fkv_standby` 引用 `fkv_types`。

### 验证

- `cargo check --workspace --all-targets --all-features` 通过。
- `cargo nextest run --workspace --all-features --no-fail-fast`：516 passed / 1 skipped。
- `cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_types/src/lib.rs:14-105` 单一来源在位（NULL_PAGE:14、PAGE_ALIGN:26、MAX_TREE_DEPTH:30、DEFAULT_*:32-38、TMP_SUFFIX:50、ValType:62、FREE_PAGE_NONE:72、encode_file_page_id:83、now_ms/now_us:100/105）；`fkv_buf/src/lib.rs:14`、`fkv_config/src/lib.rs:23` 改为导入；`fkv_large` 无 SEGMENT_ALIGN、`fkv_mapping`/`fkv_manifest` 无值编码副本（grep 零命中）。
