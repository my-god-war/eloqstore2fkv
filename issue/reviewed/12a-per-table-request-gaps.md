# 12a per-table 请求补齐：Prewarm / archive tag / listing

## 来源
`issue/done/12-runtime-modes-api.md` 阶段 1（未在阶段 2 实施）。原「缺 CleanExpired / OOM retry」两条已过期，不再列入。

## 位置

- `crate/engine/fkv_api/src/lib.rs`：`Request` / `Response` 变体，当前 25 项。
- `crate/engine/fkv_shard/src/lib.rs`：`dispatch` / `dispatch_read` / `op_of` /
  `is_read_op` / `is_write_op` / `pri_of` / `prepare_table`。
- `crate/engine/fkv_shard/src/backend.rs`：`StoreBackend::{prewarm_table, list_object}`
  （Cloud / Local / Mem 分派与显式拒绝）。
- `crate/engine/fkv_store/src/lib.rs`：`create_archive`、`prune_archives`（私有）、
  `reopen`、`list_archive_tags`。
- `crate/engine/fkv_task_prewarm/src/lib.rs`：`prewarm_table` / `prewarm_tables_with_concurrency`。
- `crate/engine/fkv_runtime/src/lib.rs`：`PrewarmCrond`、`prewarm_report`。
- `crate/io/fkv_object/src/lib.rs`：`ObjectStore::list_page`（分页列举已具备）。
- `crate/io/fkv_standby/src/lib.rs`：`StandbyService::list_remote_partition_li`。
- `crate/foundation/fkv_metrics/src/lib.rs`：`Op` / `MeteredIo::inner`。

## 缺口

1. ~~`Prewarm`：`fkv_api` 仍无 `Request::Prewarm`（per-table 请求入口缺失）~~ **已完成**（见下）。
2. ~~`ListArchiveTags`：仓库无该符号；archive 枚举只在私有 `prune_archives` 内~~ **已完成**（见下）。
3. ~~`ReopenArchive { tag }`：`Request::Reopen { tbl }` 无参数；`Store::reopen` 只读当前 manifest~~ **已完成**（archive 子集，见下）。
4. ~~`DeleteArchive { tag }`：只有 `CreateArchive` / `DeleteBranch`~~ **已完成**（archive 子集，见下）。
5. ~~`ListObject` / `ListStandbyPartition`：无对应请求与 Store 方法~~ **已完成**（见下）。
6. `FileGc` / `LocalGc` 区分：`fkv_gc` 全是纯函数，未接入 `Store`；C++ 侧是
   `RequestType::FileGc` / `LocalGc` 两个请求。fkv 目前只有合并的 `Request::Gc`
   （`Store::gc` 一次跑 data + segment），指标侧已把 `Op::Gc` 对账映射到
   `file_gc` + `local_gc`。**仍是本条剩余项**，本轮未拆请求 / 未改 GC 语义。

## 方案（已按实际落地修正）

- `fkv_api`：
  - `Request::Prewarm { tbl }` + `Response::Prewarm`（Cloud 模式同步下载本地 cache）。
  - `Request::ListArchiveTags { tbl }` + `Response::ListArchiveTags(Vec<ArchiveTagInfo>)`；
    `ArchiveTagInfo { tag, term }`，按 `(tag, term)` 升序去重。
  - `Request::ReopenArchive { tbl, tag }` + `Response::Reopen`。
  - `Request::DeleteArchive { tbl, tag }` + `Response::DeleteArchive(usize)`。
  - `Request::ListObject { tbl, prefix, token, max_keys }` +
    `Response::ListObject(ListPage)`；`prefix` 是远端原始 key 前缀（对齐 C++
    `ListObjectRequest::SetRemotePath`），`tbl` 仅用于选 shard。
  - `Request::ListStandbyPartition` + `Response::ListStandbyPartition(Vec<TableIdent>)`。
- `fkv_store`：新增公开 `list_archive_tags` / `list_archive_tag_li`（表级清单，
  跨 branch/term 按 `(tag, term)` 排序去重，不读 manifest、不要求表已打开）；
  `reopen_archive` / `delete_archive` 已在 archive 子集落地。
- `fkv_shard`：
  - `StoreBackend::{prewarm_table, list_object}`：Cloud 分支复用
    `fkv_task_prewarm::prewarm_table` 与 `fkv_object::ObjectStore::list_page`；
    Local / Mem 显式 `InvalidArgs`。
  - `dispatch_read` 处理 `ListArchiveTags` / `ListObject`；`dispatch` 处理
    `Prewarm` / `ListStandbyPartition`；`op_of` 补齐 4 个 `Op`；
    `is_read_op` / `is_write_op` / `pri_of` / `prepare_table` 同步。
  - `Prewarm` 进 per-partition 写队列（低优先）；`ListObject` 只读并发。
- `fkv_standby`：`StandbyService::{list_remote_partition_li, list_partition_li}`
  直接复用 `RemoteSource::list_table_li`；多根去重由 source 保证。
- `fkv_metrics`：新增 `Op::{Prewarm, ListArchiveTags, ListObject,
  ListStandbyPartition}`；`ListObject` / `ListStandbyPartition` 映射 C++ 标签，
  `Prewarm` / `ListArchiveTags` 无 C++ `request_type`；`MeteredIo::inner()` 供
  shard 下探 Cloud 后端专有能力。
- `fkv_runtime`：新增 `prewarm` / `list_archive_tags` / `list_object` /
  `list_standby_partition` 便捷方法。

## 验收（已满足）

- `tests/`：archive 列举 / 按 tag reopen / 删除 tag 全链路（既有）；
  `Prewarm` / `ListObject` / `ListStandbyPartition` 在 Local 返回 `InvalidArgs`；
  Cloud 下 `Prewarm` 落本地 cache、`ListObject` 列举远端对象（`EKV_S3_ENDPOINT`
  启用）；`MemObjectStore` 上验证 `list_page` 分页契约。
- `match` 穷尽保证 `Request` 变体都有 `dispatch` 分支；新增变体同步 `op_of`。
- `cargo clippy -p fkv_api -p fkv_store -p fkv_metrics -p fkv_standby -p
  fkv_shard -p fkv_task_prewarm -p fkv_runtime --all-targets --no-deps --
  -D warnings` 零警告。

## 重叠

- `fkv_task_prewarm` 的跨 shard 触发归 `issue/todo/cld-47a-prewarm-background-trigger-and-cross-shard.md`；
  本条只做「请求入口 + per-table 调用」。
- archive tag 语义/保留策略归 `issue/todo/eng-59`，本条复用其结果。
- `ListObject` 分页 token 契约见 `issue/todo/io-29*`。

## 阻塞（2026-11-05 更新：已解除）

`fkv_task_prewarm` 与 runtime 的依赖已落地（原「runtime 未依赖 prewarm」与
`runtime/src/lib.rs:336/460` 编译不过的说法均已过期）。剩余阻塞只在请求面：
`Request` 新增变体会直接打断 `fkv_shard` / `fkv_runtime` 的**穷尽** `match`
（`dispatch` / `op_of` / `is_read_op` / `is_write_op` 均无 `_` 兜底），而这两处
plus `fkv_store` / `fkv_metrics` 都不属本 lane，故只改 `fkv_api` 会让 workspace
无法编译。需要多 lane 同一批次协同。

**2026-11-05 per-table 批次已把上述 crate 一起改完**：`fkv_api` / `fkv_store` /
`fkv_shard` / `fkv_metrics` / `fkv_task_prewarm` / `fkv_standby` / `fkv_runtime`
同批编译、clippy 与 targeted tests 通过，阻塞解除。以下 owner 表保留作历史记录。

### 各 crate / owner

- `fkv_api`（本 lane）：新增 `Request::{Prewarm, ListArchiveTags, ReopenArchive,
  DeleteArchive, ListObject}` + 对应 `Response`；`Request::table` 补齐。
  **必须与下列变动同一批次落地**。
- `fkv_store`（热点 lane）：`prune_archives`（`src/lib.rs:2030`，目前私有）抽成公开
  `list_archive_tag_li` / `reopen_archive` / `delete_archive`，复用现有名字排序规则。
- `fkv_shard`（另一 lane）：`dispatch` 新分支 + `op_of` / `is_read_op` /
  `is_write_op` 补项；`Prewarm` 模式守卫（仅 `Backend::Cloud`，其它 `InvalidArgs`）；
  `ListObject` 复用 `fkv_object::ObjectStore::list_page`（`io/fkv_object/src/lib.rs:102`）。
- `fkv_metrics`（另一 lane，`foundation/fkv_metrics`）：新增 `Op` 变体 + `ALL` / 桶。
- `fkv_runtime`（另一 lane）：同名便捷方法；对 `fkv_task_prewarm` 的依赖已存在
  （`Cargo.toml` + `PrewarmCrond`），无需重复添加。

### 现状确认

- 历史（阻塞时）：`fkv_api` 无这 5 个变体；`fkv_store` 无 archive tag 列举接口。
- 2026-11-05 落地后：上述 5 个请求面变体均已存在；`Store::list_archive_tags`
  已公开；`fkv_runtime` 的 `PrewarmCrond` 与新增便捷方法均已接线。
- standby runtime wiring 见 [`12b-standby-runtime-wiring.md`](../done/12b-standby-runtime-wiring.md)（已完成）。

## DONE（2026-11-05，archive 子集）

本文件第 3、4 条（`ReopenArchive { tag }` / `DeleteArchive { tag }`）已落地：

- `fkv_api::Request::{ReopenArchive, DeleteArchive}` + `Response::DeleteArchive`；
- `fkv_store::{reopen_archive, delete_archive}`（按 tag 提升归档快照 / 幂等删除
  全部 term）；
- `fkv_shard` dispatch / `op_of` / `is_write_op` / `pri_of` / `prepare_table`
  接线；`fkv_runtime::{reopen_archive, delete_archive}` 便捷方法；
- 测试：`fkv_store/tests/branch_archive.rs::reopen_and_delete_archive`、
  `fkv_runtime/tests/archive_lifecycle.rs`。

## DONE（2026-11-05，per-table 请求面：Prewarm / ListArchiveTags / ListObject / ListStandbyPartition）

第 1、2、5 条已落地：

- `fkv_api`：
  - `Request::{Prewarm, ListArchiveTags, ListObject, ListStandbyPartition}`；
  - `Response::{Prewarm, ListArchiveTags(Vec<ArchiveTagInfo>), ListObject(ListPage),
    ListStandbyPartition(Vec<TableIdent>)}`；
  - `ArchiveTagInfo { tag, term }`、`ListPage { meta_li, next_token }` /
    `ObjectMeta { key, etag, size }`（纯数据镜像，不引入 IO 依赖）。
- `fkv_store::Store::list_archive_tags`（别名 `list_archive_tag_li`）：表级
  inventory，扫全部 branch / term 的归档 manifest，按 `(tag, term)` 升序去重；
  不读 manifest、不要求表已打开；Cloud 下由 `ManifestIo` 合并本地 cache 与远端
  列举。
- `fkv_shard`：
  - `StoreBackend::prewarm_table`（Cloud → `fkv_task_prewarm::prewarm_table`；
    Local / Mem → `InvalidArgs`）；
  - `StoreBackend::list_object`（Cloud → `ObjectStore::list_page`；其它 →
    `InvalidArgs`）；
  - `ListStandbyPartition` → `StandbyService::list_remote_partition_li`；
  - `dispatch_read` / `dispatch` / `op_of` / `is_read_op` / `is_write_op` /
    `pri_of` / `prepare_table` 全部补齐；`Prewarm` 进 per-partition 写队列、
    低优先；`ListArchiveTags` / `ListObject` 走只读并发路径。
- `fkv_standby::StandbyService::{list_remote_partition_li, list_partition_li}`。
- `fkv_metrics`：新增 `Op::{Prewarm, ListArchiveTags, ListObject,
  ListStandbyPartition}`；后两者映射 C++ `request_type`，前两者无 C++ 标签；
  `MeteredIo::inner()` 让 shard 下探后端专有能力。
- `fkv_runtime`：`prewarm` / `list_archive_tags` / `list_object` /
  `list_standby_partition` 便捷方法。
- 测试：
  - `fkv_store/tests/branch_archive.rs::list_archive_tags_is_table_scoped_sorted_and_deduped`
    （排序、去重、表隔离、跨 branch、空表）；
  - `fkv_runtime/tests/per_table_ext.rs`（Local 下 Prewarm / ListObject /
    ListStandbyPartition 均回 `InvalidArgs`，ListArchiveTags 命中 CreateArchive）；
  - `fkv_task_prewarm/tests/prewarm.rs::cloud_remote_list_object_paginates_over_mem_object_store`
    （MemObjectStore 分页契约）；
  - `fkv_runtime/tests/prewarm.rs::s3_per_table_prewarm_and_list_object_requests`
    （`EKV_S3_ENDPOINT` 启用：ListObject 列远端、Prewarm 落本地 cache）；
  - `fkv_runtime/tests/standby.rs`（replica 能列出 master 分区）。

验证命令：

```
cargo test -p fkv_metrics --test metrics
cargo test -p fkv_store --test branch_archive
cargo test -p fkv_task_prewarm --test prewarm
cargo test -p fkv_runtime --test per_table_ext
cargo test -p fkv_runtime --test standby
cargo test -p fkv_shard
cargo clippy -p fkv_api -p fkv_store -p fkv_metrics -p fkv_standby \
  -p fkv_shard -p fkv_task_prewarm -p fkv_runtime --all-targets --no-deps -- -D warnings
```

## 剩余（2026-11-05 已由下方 FileGc/LocalGc DONE 关闭，保留作历史记录）

第 6 条 `FileGc` / `LocalGc` 粒度仍未做：

- `Request::Gc` 仍是合并语义（`Store::gc` 一次跑 data + segment），没有
  `FileGc { file_id }` / `LocalGc` 的独立请求与 `Store` 方法；
- `fkv_gc` 仍是纯函数集合，未接入 `Store` 的 request 面；
- 指标侧 `Op::Gc` 已映射 C++ `file_gc` + `local_gc`，但请求 / 计数粒度仍合并。

已知限制（不阻塞本条剩余项，但调用方需知）：

1. `Prewarm` 是同步请求：整轮下载在 shard 线程内完成，大表会长时间占用该
   shard；后台周期预热仍应使用 `PrewarmCrond`。
2. `ListObject` 的 `prefix` 是远端原始 key 前缀（C++ `RemotePath` 语义），
   不做表目录自动拼接；调用方需自行传 `<cloud prefix>/<tbl>.<partition>/`。
3. `Runtime::list_standby_partition` 固定发往 shard 0：各 shard 的 standby
   service 共享同一 master 来源，逐 shard 列举只会重复；多 DS 的分区过滤归
   `sto-61b`。
4. `ListStandbyPartition` 直接列 master 根下 `<tbl>.<partition>` 目录，尚未按
   LUT / 分区过滤器收窄（与 C++ `ListRemotePartitions` 的观察语义一致）。

## DONE（2026-11-05，FileGc / LocalGc 拆分）

第 6 条已落地，12a 全部条目关闭。

C++ 语义核对（`/Users/z/git/db/kv/eloqstore`）：

- `FileGcRequest` → `WriteTask::RunFileGc`：Cloud 走 `ExecuteCloudGC`（删远端
  对象 + 本地 cache），Local 走 `ExecuteLocalGC`（只删本地文件）；
- `LocalGcRequest` → `WriteTask::RunLocalFileGc`：无条件走 `ExecuteLocalGC`，
  只删本地文件 / 本地 cache，绝不删远端对象；
- 两者都是 `WriteRequest`，进 per-partition 写队列，一轮同时处理 data 与
  segment 文件。

落地：

- `fkv_api`：`Request::{FileGc, LocalGc}` + `Response::{FileGc(usize),
  LocalGc(usize)}`；删除合并的 `Request::Gc { pages_per_file_shift }` ——
  布局位宽只取 shard / store 配置，请求不再携带（避免调用方传错位宽）。
- `fkv_io`：`FileIo` 新增 4 个本地路径能力（默认委托给整 id 方法）：
  - `list_local_file_id_li` / `remove_local_file`
  - `list_local_segment_file_id_li` / `remove_local_segment_file`
- `fkv_cloud`：云端后端覆写上述方法，只操作 `self.local` cache，**不调用
  `remote.delete`**；`FileGc` 仍走 `remove_file` / `remove_file_variant`
  （删远端对象）。
- `fkv_task_gc`：新增 `GcScope::{File, Local}`，`gc_files_with_retained_budget`
  与 `gc_segment_files_with_retained_budget` 保持原语义（File），新增
  `gc_local_files_with_retained_budget` /
  `gc_local_segment_files_with_retained_budget`（Local）；引用集 / in-flight
  guard 判定共用，只在列举与删除路径分叉。
- `fkv_store`：`gc` 更名为 `file_gc`（`FileGc` 主体，data + segment 一轮），
  新增 `local_gc`；两者共用 `gc_with_scope`。`local_gc` **跳过**
  `prune_superseded_manifest_file_li`（云端实现会删远端 manifest，本地 GC
  只处理 data / segment，C++ `ExecuteLocalGC` 同样不剪 manifest）。
- `fkv_shard`：`dispatch` 两个分支（位宽取 `state.pages_per_file_shift`）、
  `op_of` / `pri_of`（低优先）/ `is_write_op` 接线。
- `fkv_metrics`：`Op::Gc` 拆为 `Op::FileGc => "file_gc"` /
  `Op::LocalGc => "local_gc"`，与 C++ 标签一一对应（不再需要「两标签求和」）。
- `fkv_runtime`：`Runtime::{file_gc, local_gc}`（均不再接收位宽参数）。

测试：

- `fkv_cloud/tests/local_gc.rs::local_gc_deletes_local_cache_only_and_keeps_remote_objects`
  （`CloudStore<MemStore, MemObjectStore>`）：`local_gc` 后本地 cache 文件数
  下降、`fkv/<tbl>/data_*` 远端对象集合与数量完全不变；随后 `file_gc` 才把
  远端 data 对象删空。
- `fkv_store/tests/local_gc.rs::local_gc_reclaims_unreferenced_local_files`
  （Local / Mem，无远端）：`local_gc` 回收无引用文件，表仍可读写。
- `fkv_shard/tests/lifecycle.rs`：`FileGc` / `LocalGc` 两个请求都能经 dispatch
  返回对应 `Response`；`priority.rs` 的 Gc 请求改用 `FileGc`。
- `fkv_runtime/tests/modes.rs`：Local 模式下的 `file_gc` / `local_gc` 往返。
- `fkv_metrics/tests/metrics.rs`：标签映射改为一对一，`Op::ALL.len() == 21`。

验证命令（并发 lane 正在重构 `fkv_shard` / `fkv_task_gc`，以下为本次通过项）：

```
cargo test -p fkv_store --test local_gc --test gc_store --test gc_snapshot \
  --test gc_inflight --test gc_cleanup --test branch_gc --test segment_compact \
  --test truncate_drop
cargo test -p fkv_cloud --test local_gc --test term_key --test manifest_prune \
  --test large_value
cargo test -p fkv_task_gc
cargo test -p fkv_task_compact --test compact
cargo test -p fkv_metrics --test metrics
cargo test -p fkv_shard --test lifecycle --test priority
cargo test -p fkv_runtime --test modes
cargo clippy -p fkv_api -p fkv_io -p fkv_cloud -p fkv_task_gc -p fkv_store \
  -p fkv_metrics --all-targets --no-deps -- -D warnings
```

已知限制：`LocalGc` 不清理本地 manifest、也不移除云端本地 cache 目录
（`CloudStore::try_remove_empty_partition_dir` 为默认 no-op）；这两项与
`FileGc` 相同，C++ `ExecuteLocalGC` 的第 5 步目录收尾在 fkv 云端后端尚未实现。
`doc/metrics.md` 与 `doc/architecture/06-gaps.md` 仍写着 `Gc → file_gc + local_gc`，
需在文档 lane 同步（不在本次可改 crate 范围）。

## 复审（2026-11-05 r7）

**结论：通过，归档。** 三个 DONE 小节（archive 子集、per-table 请求面、
FileGc/LocalGc 拆分）的全部声称逐条对到当前源码与测试，未发现不实或回归。
文档滞后残余另立 [`rt-02`](../new/rt-02-gc-mapping-doc-sync.md)。

### 请求枚举穷尽性（重点核对项）

- `fkv_api/src/lib.rs`：`Request` 现存 25 变体含 `Prewarm`(:153)、
  `ListArchiveTags`(:164)、`ListObject`(:179)、`ListStandbyPartition`(:190)、
  `ReopenArchive`(:132)、`DeleteArchive`(:141)、`FileGc`(:65)、`LocalGc`(:74)；
  合并的 `Request::Gc` 全仓 grep 零残留；`Request::table()`(:203-230) 覆盖全部
  per-table 变体；`Response::{FileGc(usize), LocalGc(usize), DeleteArchive(usize),
  Prewarm, ListArchiveTags, ListObject, ListStandbyPartition}` 与纯数据镜像
  `ArchiveTagInfo`(:302) / `ObjectMeta`(:309) / `ListPage`(:320) 均在。
- `fkv_shard/src/lib.rs`（文件 mtime 04:33，内容与声称相符，非在途冲突）：
  - `dispatch`(:2664-3003)：入口 `is_read_op` 走 `dispatch_read`，其余 25 变体
    逐一有臂（`ReopenArchive`:2760、`FileGc`:2803、`LocalGc`:2809、
    `DeleteArchive`:2879、`Prewarm`:2885、`ListStandbyPartition`:2897），尾部
    只读臂(:2993-3001)仅为 match 穷尽；
  - `dispatch_read`(:2153-2239)：`ListArchiveTags`(:2199)、`ListObject`(:2211)；
  - `op_of`(:1145-1180)：8 个新变体全映射（`ReopenArchive`→`Reopen`、
    `DeleteArchive`→`CreateArchive`，注释给出 C++ RequestType 依据）；
  - `is_read_op`(:1426) 含 `ListArchiveTags`/`ListObject`；`is_write_op`(:1444)
    含 `FileGc`/`LocalGc`/`ReopenArchive`/`DeleteArchive`/`Prewarm`；
  - `pri_of`(:86)：`FileGc`/`LocalGc`/`Prewarm`/`ListObject` → Low；
  - `skips_table_open`(:1637)/`prepare_table`(:1648)：控制面请求不懒建空表。
- `fkv_metrics/src/lib.rs:104-126`：`FileGc => "file_gc"`、`LocalGc => "local_gc"`
  一对一；`Prewarm`/`ListArchiveTags` 空 C++ 标签、`ListObject`/`ListStandbyPartition`
  有映射；`MeteredIo::inner`(:594) 供 shard 下探。`Op::Gc` 零残留。

### FileGc / LocalGc 落地形态与 C++ 对照（不臆造语义项）

- C++ 亲读：`write_task.cpp:967` `RunFileGc`（Cloud→`ExecuteCloudGC`、否则
  `ExecuteLocalGC`，data+segment 同轮）；`:1035` `RunLocalFileGc`（无条件
  `ExecuteLocalGC`）；`eloq_store.h:792/801` 两请求均 `WriteRequest` 子类
  （进 per-partition 写队列）。与 DONE 的语义描述逐条一致。
- Rust：`fkv_task_gc/src/lib.rs:708` `GcScope::{File,Local}`，列举/删除按 scope
  分叉（:714-776），引用集与 in-flight guard 共用；`fkv_store/src/lib.rs:3787`
  `gc_with_scope`，`local_gc` 跳过 `prune_superseded_manifest_file_li`
  （:3911-3918 注明对齐 C++ 不剪 manifest）；`fkv_io/src/lib.rs:588-626` 四个
  本地路径方法（默认委托整 id）；`fkv_cloud/src/lib.rs:2905-2969` 云端覆写只碰
  `self.local`，无 `remote.delete`，而 `FileGc` 路径 `remove_segment_file_variant`
  （:2901）有 `remote.delete`。位宽取 `state.pages_per_file_shift`
  （shard `dispatch:2801-2814`），请求不携带。
- 测试抽读：`fkv_cloud/tests/local_gc.rs:79-102`（远端对象集合逐 key 相等不变、
  本地 cache 缩小、随后 `file_gc` 清空远端 data）、`fkv_store/tests/local_gc.rs`
  全文（回收 + 表仍可读写）、`fkv_shard/tests/lifecycle.rs:270-280`（两请求经
  dispatch 回对应 Response）、`fkv_runtime/tests/modes.rs:69-70`、
  `fkv_metrics/tests/metrics.rs:127`（`Op::ALL.len() == 21`）、
  `priority.rs:98`（改用 `FileGc`）。全部与声称一致。

### 其余请求族测试与 eng-84 边界

- `list_archive_tags`：`fkv_store/tests/branch_archive.rs:212`（乱序 tag 排序、
  跨 branch 可见、表隔离、空表）；`per_table_ext.rs:56`（Local 下
  Prewarm/ListObject/ListStandbyPartition 回 `InvalidArgs`、控制面请求不建目录、
  ListArchiveTags 命中归档）；`fkv_task_prewarm/tests/prewarm.rs:285`（分页
  token 契约）；`fkv_runtime/tests/prewarm.rs:226`（S3 门控 e2e）；
  `standby.rs:231`（多 master 根分区发现）；`archive_lifecycle.rs` /
  `branch_archive.rs::reopen_and_delete_archive`(:326)（archive 子集，含重启
  后仍是归档快照的盘面断言）。
- eng-84 边界清晰：eng-84 todo「仍未完成」第 3 条明确把 per-table
  `ListArchiveTags` 留给 12a 第 2 条（本条已交付）；eng-84 残余只剩全局
  `GlobalReply::name` 空目标问题，本条未触碰。
- 唯一残余：`doc/metrics.md:28,40-41` 与 `doc/architecture/06-gaps.md:37` 仍写
  「Gc 合并 / fkv 无 list_object 请求」，与代码相反（DONE 已自述，非隐瞒）；
  无 open issue 承接 → 已立 `rt-02`。cargo/clippy 通过声称本轮无法复跑
  （复审禁 cargo），以代码 + 测试文本核对为准。
