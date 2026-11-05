# 12 P0：Runtime 只接 Local，Cloud/Standby 与一批请求类型未暴露

## 现象

`Shard::start` 固定构造 `CompioStore`：

```rust
// crate/engine/fkv_shard/src/lib.rs:111
CompioStore::new(config.root, config.page_size, config.pages_per_file_shift)
```

- 没有 `CloudStore` / `StandbyService` 的运行时构造
- `RuntimeConfig` 只有 num_threads/root/page_size 等本地字段
- `KvOption` 的 `store_path`、`cloud_*`、`enable_local_standby` 没有驱动 Runtime
- `fkv_api::Request` 只有 Put/Get/Scan/Reopen/Gc/Compact/Truncate/Floor/Drop/Archive/Branch/Attach/Detach

缺少：

- `CleanExpired`
- `FileGc` / `LocalGc` 区分
- `Prewarm`
- `ListArchiveTags`
- `GlobalArchive` / `GlobalReopen` / `GlobalCreateBranch` / `DropTable` 全局 fanout
- Archive delete action
- `Reopen(tag/clean)`
- `ListObject` / `ListStandbyPartition`
- OOM retry

## C++ 对照

`docs/architecture/03-request-api.md` 列出了完整的请求 taxonomy，包括 remote listing、global fanout、file/local GC、clean expired、prewarm、archive action。C++ Runtime 通过 `StoreMode` / `KvOption` 选择 Local/Mem/Cloud/Standby。

## 影响

- 库级测试覆盖了 Cloud/Standby，但用户无法通过 Runtime/SDK 使用
- API 对齐度不足，C++ 调用方迁移会卡住
- `CleanExpired`、`Prewarm` 等只能直接调 Store，无法进入 shard 队列
- 云/standby 的 auto-reopen 无法真正生效

## 建议

1. 在 shard 引入 IO backend 枚举：
   - `Local(CompioStore)`
   - `Cloud(CloudStore<CompioStore, S3Store>)`
   - `Standby(...)`
2. `RuntimeConfig` 增加 mode/cloud/standby/prewarm/cache 字段，或直接接受 `KvOption`。
3. 补齐 `Request` 枚举和 `handle` 分支，先做 per-table，再做 runtime 级 global fanout。
4. 增加与 C++ 请求类型的对照表测试。

## 验收

- Runtime 能以 Cloud/Standby 模式启动并完成 put/get/reopen
- CleanExpired/Prewarm/ListArchiveTags 可通过 Runtime API 调用
- 请求类型覆盖表与 C++ 一致（允许明确标注的未实现项）

## 审查结论（2026-11-04）

### 总结论：部分采纳

核心事实成立：**运行时只有本地一种装配**，`Cloud` / `Standby` 虽有可用实现与库级测试，但没有任何运行时入口。
且这属于 1.0 范围（`todo.md`：1.0 = `Local + Mem + Cloud + Standby`；`COMPAT_BASELINE` 的 `L1` 只是当前兼容等级，不是范围裁剪）。

但原 issue 的**缺失清单已过期**：`CleanExpired`、`OOM retry` 早已实现；`DropTable` 以 `Runtime::drop_table` 存在；C++ 对照路径不存在，无法核验。
因此按「部分采纳、修正清单后开发」处理，移入 `issue/todo/`。

### 逐条核验表

| # | 原 issue 主张 | 核实结果 | 证据 |
|---|---|---|---|
| 1 | `Shard::start` 固定构造 `CompioStore` | **属实** | `crate/engine/fkv_shard/src/lib.rs:124` `CompioStore::new(...)`；且 `ShardState.store: Store<MeteredIo<CompioStore>>`（同文件 `:337`）是具体类型，不是泛型/枚举 |
| 2 | 无 `CloudStore` 运行时构造 | **属实** | `crate/engine/fkv_cloud/src/lib.rs:70` `pub struct CloudStore<I, O>`；`fkv_runtime/Cargo.toml` 未依赖 `fkv_cloud` |
| 3 | 无 `StandbyService` 运行时构造 | **属实** | `crate/io/fkv_standby/src/lib.rs` 提供 `StandbyConfig`/`RemoteSource`；`fkv_runtime` 未依赖 `fkv_standby` |
| 4 | `RuntimeConfig` 只有本地字段 | **属实** | `crate/engine/fkv_runtime/src/lib.rs:32-49` 仅 `root/page_size/segment/queue/auto_reopen/archive` 等；无 mode/cloud/standby/prewarm/cache |
| 5 | `KvOption` 的 `store_path`/`cloud_*`/`enable_local_standby` 未驱动 Runtime | **属实** | `start_with_options` 只映射压缩/checksum/overflow/OOM/manifest 等（`:226-244`）；`rg store_path\|cloud_store_path\|enable_local_standby` 在 `fkv_runtime`/`fkv_sdk`/`fkv_cli` 无命中 |
| 6 | `Request` 只有 13 项 | **不准确（过期）** | `crate/engine/fkv_api/src/lib.rs:7-84` 实有 `Put/Get/PutLarge/GetLarge/Scan/Reopen/Gc/Compact/Truncate/Floor/Drop/CleanExpired/AttachShard/DetachShard/CreateArchive/CreateBranch/DeleteBranch/Shutdown`（18 项） |
| 7 | 缺 `CleanExpired` | **不成立** | `Request::CleanExpired`（`fkv_api:59`）+ `Runtime::clean_expired`（`fkv_runtime:630`）+ `tests/ttl_cleanup.rs` |
| 8 | 缺 `OOM retry` | **不成立** | `fkv_shard/src/lib.rs:181-209` 重试循环 + `KvOption.auto_oom_retry_times`（`fkv_config:82`）；残留问题是「重试分支无条件 `clone`」，已在 `eng-56` 跟踪 |
| 9 | 缺 `Prewarm` 请求 | **属实** | `crate/engine/fkv_task_prewarm/src/lib.rs` 有 `prewarm_table`，但 `fkv_api` 无 `Prewarm`，`fkv_runtime` 未依赖 `fkv_task_prewarm` |
| 10 | 缺 `ListArchiveTags` | **属实** | 仓库内无该符号；`Store` 只在私有 `prune_archives` 内枚举 archive（`fkv_store:1723`），无公开列举接口 |
| 11 | 缺 archive delete action | **属实** | 只有 `CreateArchive` / `DeleteBranch`；无删除单个 archive tag 的请求或 `Store` 方法 |
| 12 | 缺 `Reopen(tag/clean)` | **属实** | `Request::Reopen { tbl }` 无参数；`Store::reopen`（`fkv_store:1240`）只读当前 `manifest_file`，不能指定 tag |
| 13 | 缺 `GlobalArchive`/`GlobalReopen`/`GlobalCreateBranch` 全局 fanout | **属实（API 层）** | `fkv_api`/`fkv_runtime` 无 `global_*` 方法；只有 `ArchiveCrond::tick` 内部对所有 shard fanout `CreateArchive`（`fkv_runtime:120-160`） |
| 14 | 缺 `DropTable` 全局 fanout | **部分不成立** | `Runtime::drop_table`（`:640`）已存在，但是 per-table；缺的是「全部 shard / 全部表」语义 |
| 15 | 缺 `ListObject` / `ListStandbyPartition` | **属实** | 仓库内无 `ListObject`/`ListStandby` 符号；`CloudStore` 只有 `remote_data_file_id_li` 等内部辅助（`fkv_cloud:979-1004`） |
| 16 | `FileGc` / `LocalGc` 区分缺失 | **无法核验，且证据不足** | `fkv_gc` 全是纯函数（`retained_file_page_set`/`pick_compact_file`），未接入 `Store`；仓库无 C++ 头文件可对照，`docs/architecture/03-request-api.md` 不存在 |
| 17 | 「C++ Runtime 通过 `StoreMode`/`KvOption` 选择四种模式」 | **Rust 侧半成品属实** | `fkv_config::derive_mode()`（`:160-172`）已能判 `Cloud`/`StandbyMaster`/`StandbyReplica`，但无人消费 |
| 18 | 「库级测试覆盖了 Cloud/Standby」 | **属实** | `crate/engine/fkv_cloud/tests/`（`cloud.rs`、`branch_archive.rs`、`s3_rustfs.rs` 等）、`crate/io/fkv_standby/tests/standby.rs` |
| 19 | C++ 对照文档路径 `docs/architecture/03-request-api.md` | **不存在** | 仓库只有 `doc/architecture/01..06`，无 request-api；`cpp/eloqstore` 为空目录，无法做类型对照 |
| 20 | （原 issue 未提）缺 Mem 模式 | **新发现，同样缺失** | `fkv_io_mem::MemStore` 存在，`fkv_runtime` 未依赖；`todo.md` 把 `Mem` 列入 1.0 范围 |

结论：**19 条中 13 条属实**；3 条过期或不成立（6、7、8）、1 条部分不成立（14）、1 条无法核验（16）、1 条引用路径不存在（19）；另有 1 条遗漏（Mem 模式）。
核心问题（运行时无 IO 后端抽象）成立，须做；清单须按上表修正后执行。

### 分阶段开发计划（采纳）

**阶段 0：修正 issue 清单（0.5 天）**

- 删除 `CleanExpired`、`OOM retry` 两条（改挂 `eng-56`）。
- `DropTable` 改为「全局 fanout 缺失」，不是「请求缺失」。
- 补 `Mem` 模式，与 Cloud/Standby 同批验收。
- 删除不可核验的 C++ 文档引用，改为「以 `cpp/shim` + fixture 对照为准，待 `01` 落地」。

**阶段 1：per-table 请求补齐（1-2 天，无运行时结构改动）**

- `fkv_api::Request` 增 `Prewarm`、`ListArchiveTags`、`ReopenArchive { tag }`、`DeleteArchive { tag }`；`Response` 增对应回执。
- `fkv_store` 暴露 archive 列举/按 tag 打开/删除，复用现有 `prune_archives` 的文件名解析与保留策略。
- `fkv_shard::handle` 加分支；`fkv_runtime` 加同名便捷方法。
- 约束：`Prewarm` 在非 Cloud 模式返回 `InvalidArgs`，不静默降级。
- 验收：`tests/` 覆盖 archive 列举/按 tag reopen/删除 + prewarm 模式守卫。

**阶段 2：IO 后端抽象（3-5 天，关键路径）**

- 定义 `enum IoBackend { Local(CompioStore), Mem(MemStore), Cloud(CloudStore<CompioStore, S3ObjectStore>) }`，为它实现 `PageIo/ManifestIo/FileIo/SegmentIo/FixedSegmentIo/TermIo` 六件套转发；`MeteredIo<IoBackend>` 复用现有实现。
- `ShardState.store` 由 `Store<MeteredIo<CompioStore>>` 改为 `Store<MeteredIo<IoBackend>>`；`ShardConfig` 增加 `mode` 与 cloud/standby 参数（或直接持 `KvOption`）。
- `RuntimeConfig` 增加 `mode`/`cloud`/`standby` 字段；`start_with_options` 改为按 `options.derive_mode()` 构造，`store_path`/`cloud_*`/`enable_local_standby` 一次性接通。
- 保持热路径无 `dyn`（enum dispatch，符合 `todo.md` 硬约束 4）。
- 验收：`Runtime::start_with_options` 以 Local/Mem/Cloud 三模式跑通 put/get/reopen/scan；Local 路径零行为变化（现有 runtime 测试全绿）。

**阶段 3：Standby 与 auto-reopen/prewarm（2-3 天）**

- Standby master/replica 用独立同步线程（rsync/ssh 是阻塞外部进程，严禁进 shard 线程）。
- Cloud 模式 attach 后触发 `prewarm_table`（`prewarm_cloud_cache`/`prewarm_task_count` 已在 `KvOption`）。
- 云 auto-reopen 走现有 `AutoReopenOption` 重试，`CloudStore` 的远端 manifest 拉取已在库层验证。
- 验收：Standby replica 从 master 追上并 serve 读；Cloud 模式 kill 本地 cache 后 reopen 自动恢复。

**阶段 4：全局 fanout（2 天）**

- `GlobalArchive`/`GlobalReopen`/`GlobalCreateBranch`/`GlobalDropTable` 复用 `ArchiveCrond::tick` 的「顶层线程 `block_on` + `join_all` 投递」模式，禁止在 shard poll 栈内驱动调度器。
- 分批上限用 `max_global_request_batch`。
- 验收：多 shard 下 fanout 覆盖全部已 attach 分区，单 shard 失败可观测（不整体静默）。

**阶段 5：对照表测试（1 天）**

- 建「请求类型 × 模式 × 支持状态」表，编译期或测试期断言 `Request` 变体全覆盖 `handle` 分支（`match` 穷尽已保证），未实现项显式标注。
- 与 C++ 对照延后到 `01` 的真实 oracle 就绪后再补齐。

### 风险

1. **泛型波及面**：`Store<I>` 的 trait bound 由 6 个 IO trait 叠加，`IoBackend` 必须逐 trait 转发，漏一个就编译失败；收益是编译期单态化 + 无 `dyn`。
2. **Cloud 的 fixed IO 条件 bound**：`CloudStore` 的 `FixedSegmentIo` 仅在内层 `I: FixedSegmentIo` 时成立，`register_fixed_buffer` 需按模式 gating，否则 Cloud 模式启动即失败。
3. **S3 自带后台线程**：`S3ObjectStore` 内有命令线程与自己的 `RuntimeConfig`，shard 线程只管 async 收包；禁止在 shard poll 栈内 `block_on` 等它。
4. **Standby 阻塞进程**：rsync/ssh 必须放独立线程或阻塞池，进 shard 会直接违反 thread-per-core 纪律。
5. **term CAS 仍是桩**：`TermIo` 未实现真 CAS（见 `issue/new/25` 相关条目），多写仲裁类验收（Cloud/Standby 双写切换）当前不可闭环，需在验收里标注为「未实现」。
6. **Mem 模式语义**：`MemStore` 无 durability，`Reopen`/archive 语义要么 no-op 要么 `InvalidArgs`，须显式定义，避免与 Local 混同。
7. **模式爆炸**：4 模式 × 18 请求的组合测试成本高；建议按「能力矩阵」表驱动测试，避免手写 N×M。
8. **顺序风险**：阶段 2 改动 `ShardState` 会与 `03`/`eng-32`（shard 并发化）、`eng-56`（重试 clone）冲突，建议阶段 2 排在并发重构之前或与之合并评审。

## 阶段 2 落地结果（2026-11-04）

已落最小闭环：Storage 抽象 + Runtime 按 Mode 构造后端（enum 分派，无 `dyn`）。

### 改动

- `crate/engine/fkv_shard/src/backend.rs`（新增）
  - `Backend { Local, Mem, Cloud(CloudBackend) }`：shard 侧装配参数。
  - `StoreBackend { Local(Box<CompioStore>), Mem(Box<MemStore>), Cloud(Box<CloudStore<CompioStore, S3ObjectStore>>) }`：
    逐条转发 `PageIo` / `ManifestIo` / `SegmentIo` / `FileIo` / `TermIo` / `FixedSegmentIo`，
    `MeteredIo<StoreBackend>` 复用既有指标包装，热路径零虚调用。
  - `split_cloud_store_path`：解析 `cloud_store_path = bucket/prefix`。
- `crate/engine/fkv_shard/src/lib.rs`
  - `ShardConfig.backend: Backend`（默认 `Local`）。
  - `build_backend` 在 shard 线程内装配；初始化失败经 `ready_tx` 回传，`Shard::start` 报错。
  - `ShardState.store` 改为 `Store<MeteredIo<StoreBackend>>`。
- `crate/engine/fkv_runtime/src/lib.rs`
  - `backend_of(options)`：`KvOption::derive_mode()` → Local / Cloud（`cloud_store_path` +
    `cloud_endpoint/region/access_key/secret_key`）/ Standby。
  - Standby master/replica 显式返回 `InvalidArgs`，不再静默按 Local 启动。
- 测试
  - `fkv_shard/tests/backend.rs`：Mem 后端 attach/put/get/reopen，且不落盘。
  - `fkv_runtime/tests/modes.rs`：Local put/get/reopen、Cloud 装配与关闭、Standby 显式拒绝、
    Cloud 非法 `cloud_store_path` 拒绝。

### 验收状态

- [x] `Runtime::start_with_options` 按 Local / Cloud 装配，Local 行为不变。
- [x] Mem 后端可经 `ShardConfig.backend` 使用（尚未暴露到 `KvOption`，`StoreMode` 无 Mem）。
- [x] `cargo check -p fkv_runtime -p fkv_shard -p fkv_sdk` 通过。
- [x] `cargo nextest run -p fkv_runtime -p fkv_shard -p fkv_sdk -p fkv_store` 118 全绿。
- [x] `cargo clippy -p fkv_runtime -p fkv_shard -p fkv_sdk --no-deps --all-features --all-targets -- -D warnings` 零警告。
- [x] Cloud 真实读写待 S3 环境（沿用 `fkv_cloud/tests/s3_rustfs.rs` 的 `EKV_S3_ENDPOINT` 门槛）。
- [ ] Standby 执行器、请求补齐、global fanout 见 `issue/todo/12-*.md`。

### 与并行 issue 的重叠

- `issue/todo/eng-39c-cloud-standby-auto-reopen-mode.md`：其阻塞项「`RuntimeConfig` / `ShardConfig`
  无 mode」已解除（`ShardConfig.backend` 已存在）；剩余 auto-reopen 读请求重排仍归 eng-39c。
- `issue/todo/cld-47a`：Cloud 下 prewarm 触发仍待其落地，本次未接。
- `issue/todo/eng-59`：archive tag 语义不受影响。

## DONE（2026-11-04）

## 复审（2026-11-05 r6）

**通过。** 阶段 2 落地与测试在当前源码核实：

- `fkv_shard/src/backend.rs:65-118`：`Backend::{Local,Mem,Cloud,LocalRouted}` 与 `StoreBackend` 枚举分派（逐 trait 转发，含 `refresh_manifest`/`drop_local_manifest`），无 `dyn`；`fkv_shard/src/lib.rs:221` `build_backend(config, metrics)`。
- `fkv_runtime/src/lib.rs:872` `backend_of`：Local/Cloud 按 `derive_mode` 装配，standby 两模式转 `Backend::Local`（数据面本地 + 复制服务，见 12b）。
- 测试：`fkv_shard/tests/backend.rs:70` `mem_backend_put_get_reopen`（:111 断言 Mem 不落盘）；`fkv_runtime/tests/modes.rs:53-136` 五条（Local put/get/reopen、Cloud 启停、standby 启停、replica 缺 master 路径拒绝、非法 cloud_store_path 拒绝）。
- 遗留项（per-table 请求补齐/global fanout/Standby）分别由 12a(留 todo)/12c/12b 承接并 done，链路闭合。
