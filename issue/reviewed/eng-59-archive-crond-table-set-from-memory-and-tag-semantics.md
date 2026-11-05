# eng-59 定时归档的表集合来自内存观察而非盘面列举；KvOption 归档参数未读；archive tag 语义不同

## 位置
- Rust: `crate/engine/fkv_runtime/src/lib.rs:51-58`（`ArchiveState { table_set: PapayaSet }` 只收录本进程 `request()` 见过的表）；`:110-133`（`tick` 只对 `table_set` 内的表投递 `CreateArchive`）；`:180-186`（crond tag 为 20 位定宽毫秒 `format!("{now_ms:020}")`）；`:200-242`（`start_with_options` 不读 `archive_interval_secs/num_retained_archives`，须另传 `RuntimeConfig` 同义字段）；`crate/engine/fkv_store/src/lib.rs:41-46,1590-1593`（`is_safe_tag` 要求非空且仅 `[A-Za-z0-9._-]`，空 tag → `InvalidArgs`）
- C++: `src/eloq_store.cpp:1687-1737`（`HandleGlobalArchiveRequest` 每个 interval 重新列举 `store_path` 下全部 partition 目录，含从未写过的表，逐个 fan-out）；`src/tasks/background_write.cpp:586-591`（`tag.empty() ? std::to_string(UnixTs<microseconds>())` —— 空 tag 自动生成微秒十进制时间戳）；`src/eloq_store.cpp:898-906`（crond 由 KvOption 的 `data_append_mode && num_retained_archives>0 && archive_interval_secs>0` 驱动）

## 现状
1. Runtime 重启后，盘上已有数据但本进程未写入的表（只读表）不会被定时归档 —— 归档保护静默丢失；
2. 空 tag 用户在 C++ 得到自动 tag，fkv 报 `InvalidArgs`；
3. tag 格式不同（20 位定宽毫秒 vs 不定宽微秒）影响跨语言部署时按 tag 排序的保留淘汰顺序。

## 差异与风险
只读表长期无归档副本；同配置 ini 两边行为不同。

## 对齐建议
- crond tick 前先列盘（扫根目录 partition 名，复用 `list_manifest_file_name_li`）合并 `table_set`；
- `create_archive` 空 tag 自动生成微秒时间戳（对齐 C++ 格式）；
- `start_with_options` 从 KvOption 读归档三参数。

## 严重度
P1（归档保护静默丢失）/ P2（tag 细节）

## 核验记录（2026-11-04）
Rust 侧 `ArchiveState`/`tick`/`archive_tag`/`is_safe_tag` 实读确认；C++ 列盘 fan-out 与空 tag 自动生成引用自子代理摘录（含注释），开发时复核。

## 审查结论（2026-11-04）

### 逐条核验（对照当前源码）

| 论断 | 证据（改前） | 判定 |
| --- | --- | --- |
| `ArchiveState.table_set` 只收录本进程 `request()` 见过的表 | `fkv_runtime/src/lib.rs:51-81`：`note` 仅在 `Runtime::request` 的非只读分支调用，`forget` 仅在 Drop | 属实 |
| `tick` 只对 `table_set` 内的表投递 `CreateArchive` | 改前 `:110-133` | 属实 |
| crond tag 为 20 位定宽毫秒 | 改前 `:180-186` `format!("{now_ms:020}")` | 属实 |
| `start_with_options` 不读 KvOption 归档参数 | `:200-204` 归档参数只取 `config`；`KvOption` 已有 `archive_interval_secs` / `num_retained_archives` / `max_archive_tasks` | 属实 |
| 空 tag → `InvalidArgs` | `fkv_store/src/lib.rs:41-46 is_safe_tag` + `:1657` | 属实 |
| C++ 每轮列盘 fan-out / 空 tag 自动生成微秒 / crond 由 KvOption 驱动 | `cpp/` 只有 `build`/`shim`，无 include/src | 采信语义依据 |

### 处置

1. **列盘归档：采纳**。tick 改为 `fs::read_dir(root)` 解析 `{tbl_name}.{partition_id}`
   目录名（对齐 C++ `HandleGlobalArchiveRequest`）。同时**删除内存 `table_set`/`note`/`forget`**：
   写过的表必在盘上有目录，内存集合是冗余；而它还会把「只做过维护请求、从未落盘」的表
   登记进来，导致 crond 为不存在的表创建空目录 + 空 manifest（原 `Runtime::request` 注释
   里已承认这个风险，只是用 `read_only` 白名单绕开）。删除后 `Runtime::request` 退化为纯转发。
2. **空 tag 自动生成：采纳，但格式改为定宽 20 位微秒**（不是 C++ 的不定宽十进制）。
   `fkv_store::prune_archives`（`:1786-1812`）按 tag 字符串降序裁剪，排序必须等于时间序：
   不定宽时 `"1756990000000"`（13 位）与 `"17569900000000000"`（17 位）会错序，旧归档
   可能被保留而新归档被删。同时把 crond 的 tag 从「定宽毫秒」改成「定宽微秒」：两种单位
   混排即使都定宽也会互相错序（ms 约 13 位、µs 约 16 位，补零后短单位恒排在前）。
   统一放在 `fkv_api::archive_tag()`，crond 与空 tag 共用。
3. **KvOption 归档参数：采纳**（`archive_interval_secs` + `num_retained_archives`，有
   KvOption 时以它为准，与 checksum/压缩/OOM 等既有 KvOption 旋钮同规则）。
   `max_archive_tasks` 仍未接：当前 crond 对本轮全部分区 `join_all` 无并发上限，
   接它需要队列/扇出限流设计，超出本 issue 范围。

## DONE（2026-11-04）

### 实现要点

- `fkv_api::archive_tag()`：Unix 微秒定宽 20 位十进制，crond 与空 tag 共用。
- `fkv_runtime`：
  - `ArchiveState` 去掉 `table_set`，新增 `root`；删除 `note`/`forget`；
    `Runtime::request` 退化为直接转发（去掉只读白名单与 Drop 时的 forget）。
  - 新增 `scan_table_li(root)`：列盘 + 解析分区目录名，失败/不合规条目跳过。
  - `tick` 以列盘结果为准，仍只归档 attached 的 shard。
  - `start_impl`：有 KvOption 时归档参数取 KvOption 的 `archive_interval_secs` /
    `num_retained_archives`，否则取 `RuntimeConfig`。
  - 依赖清理：`papaya`、`gxhash` 已无使用，用 `cargo remove` 删除。
- `fkv_shard`：`Request::CreateArchive` 空 tag 时用 `fkv_api::archive_tag()` 生成，
  非空 tag 仍原样透传（零拷贝，不 clone）。

### 改动文件

- `crate/engine/fkv_api/src/lib.rs`
- `crate/engine/fkv_runtime/src/lib.rs`
- `crate/engine/fkv_runtime/Cargo.toml`（cargo remove papaya/gxhash）
- `crate/engine/fkv_shard/src/lib.rs`
- `crate/engine/fkv_runtime/tests/archive_crond.rs`

### 测试

- `cargo nextest run -p fkv_runtime --no-fail-fast`：26 passed，1 skipped。
- 新增用例（旧实现下必然失败）：
  - `archive_crond_archives_disk_partitions_never_written_here`：盘上已有、本进程只读
    （不写）的分区也会被归档。
  - `create_archive_empty_tag_generates_timestamp`：空 tag 得到 20 位十进制 tag 并写进文件名。
  - `archive_crond_follows_kv_options_archive_params`：`RuntimeConfig` 间隔为 0、
    KvOption 间隔为 1 时 crond 仍启动。
- `cargo clippy -p fkv_api -p fkv_shard -p fkv_runtime --no-deps --all-targets -- -D warnings`：0 警告。

## 复审（2026-11-05 r6）

结论：**通过**。

- `fkv_api/src/lib.rs:12-14` `archive_tag()`：Unix 微秒定宽 20 位（`format!("{:020}", now_us())`），注释写明 prune 排序=时间序动机。
- `fkv_runtime/src/lib.rs:82-89` `ArchiveState` 已无 `table_set`（attached/target/interval/num_retained/batch）；`:137` `scan_table_li` 列盘解析分区目录；`:388` `ArchiveTarget::Local` 走列盘；`:343` crond tag 用 `fkv_api::archive_tag()`。
- KvOption 归档参数接线：`:1184-1191` `merge_field("archive_interval_secs"/"num_retained_archives", ..., options)`。
- shard 空 tag 自动生成：`fkv_shard/src/lib.rs:2342-2345`。
- 测试：`fkv_runtime/tests/archive_crond.rs:216`（盘上分区本进程未写也归档）、`:246`（空 tag 20 位）、`:276`（KvOption 参数生效）。断言真实覆盖。
