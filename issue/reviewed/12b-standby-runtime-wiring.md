# 12b Standby 模式接入 Runtime（同步执行器）

## 来源
`issue/done/12-runtime-modes-api.md` 阶段 3。当前 `Runtime::backend_of` 对
`StoreMode::StandbyMaster` / `StandbyReplica` 显式返回 `InvalidArgs`，不静默降级。

## 位置

- `crate/engine/fkv_runtime/src/lib.rs`：`backend_of`（Standby 分支拒绝处）。
- `crate/io/fkv_standby/src/lib.rs`：`StandbyService`、`StandbyConfig::from_options`、
  `StandbyRole::from_mode`、`rsync`（`std::process` 阻塞）。
- `crate/engine/fkv_shard/src/backend.rs`：`Backend` 只有 Local/Mem/Cloud，数据面均为本地。
- `crate/foundation/fkv_config/src/lib.rs`：`derive_mode` / `standby_*` 字段。

## 缺口

1. Runtime 无 standby 同步线程；`StandbyService` 只在 `fkv_standby/tests` 使用。
2. `enable_local_standby` / `standby_master_addr` / `standby_master_store_paths` 未驱动 Runtime。
3. 无 replica 追赶后的 attach/term fencing 编排（`target_term` 语义）。

## 方案

- `Runtime::start_with_options`：`StandbyMaster` / `StandbyReplica` 时数据面仍用
  `Backend::Local`，另起独立线程跑 `StandbyService`（禁止进 shard poll 栈，rsync 阻塞）。
- `StandbyConfig::from_options(options, config.root)` 直接复用；`role` 由 `derive_mode` 决定。
- replica：定时 `sync_table`（或 `run` 循环）按 `standby_max_concurrency` 限流，成功后触发
  `Request::Reopen`/attach；fencing 用 `target_term`，term 落后则拒绝 install（`check_manifest_term`）。
- master：导出本地文件列表供 replica 拉取；`StandbyService::export_table` 已具备。
- 生命周期：`Runtime::shutdown` 先停 standby 线程再停 shard（反向启动顺序）。
- 启动失败必须让 `Runtime::start_with_options` 返回错误，不返回假成功。

## 验收

- `fkv_runtime/tests/standby.rs`：本地双 Runtime（master 路径 + replica 路径），
  master 写入后 replica 追上并 serve 读；停 master 后 replica 读不丢已同步数据。
- replica 以更小 term 启动时拒绝安装远端 manifest（`ExpiredTerm`/`InvalidArgs` 可观测）。
- 同步线程不阻塞 shard：master 高写压期间 replica 侧 shard 延迟指标无线性增长。
- `cargo clippy ... -D warnings` 零警告。

## 重叠/阻塞

- `issue/todo/eng-39c-cloud-standby-auto-reopen-mode.md`：mode 已由 12 落地
  （`ShardConfig.backend`），该条的「读请求遇 `ResourceMissing` 挂 waiter」仍需它完成。
- `issue/done/cld-44-*`（term fencing / manifest 选择）、`issue/done/cld-45-*`
  （rsync per-file 阻塞 vs C++ 异步）为前置结论，实施前先复核。
- 阻塞：真实 rsync/ssh 环境；无外部环境时用 `CommandRunner` 注入假实现做集成测试。

## 进展（2026-11-04，replica 追赶原语已落地）

已落地并验证（`crate/io/fkv_standby`）：

1. **表发现**（replica 靠它知道自己要追哪些分区，原来没有）：
   - `list_table_li(root)`：列存储根下 `<tbl>.<partition>` 目录，跳过 `.standby`
     staging、非表目录与怪名字；根不存在视为空。
   - `parse_table_name` / `table_li_of_name_li`：本地列举与远端 `ls` 输出共用同一套
     过滤与定序（`TableIdent` 没有 `Ord`，按 (表名, 分区号) 定序）。
   - `RemoteSource::list_table_li`：`LocalDirSource` 列根目录；
     `RsyncSource` 走一次 `ls -1`（配了 `ssh_addr` 则 `ssh <addr> ls -1`）。
     注：没用 `rsync --list-only`，它只保留普通文件行，拿不到目录名。
2. **一轮完整追赶** `StandbyService::catch_up()`：发现表 → `sync_all`（按
   `max_concurrency` 限流）→ 返回 `Vec<(TableIdent, SyncStats)>`；非 Replica 身份报
   `InvalidArgs`。单表失败整轮返回错误（副本静默半同步比报错更危险），重试节奏交给调用方。

验证：
- `cargo nextest run -p fkv_standby` 17 passed；新增 4 个：
  `catch_up_discovers_and_syncs_every_table`（master 两分区 → 一轮全追到、replica 可读、
  第二轮 data 全跳过且 manifest 仍安装）、`list_table_li_skips_staging_and_non_table_dirs`、
  `catch_up_rejects_master_role`、`rsync_list_table_li_uses_single_ls`（断言只起一次
  `ssh user@host ls -1`）。
- `cargo clippy -p fkv_standby --all-targets --no-deps -- -D warnings` 零警告；
  `cargo check --workspace --all-targets` 通过。

## 阻塞（Runtime 接线仍差一步，故意不放开）

`backend_of` 目前仍对 `StandbyMaster`/`StandbyReplica` 报 `InvalidArgs`，**有意保持**：
放开它而不起追赶线程，就等于副本永远不追赶却对外报成功——正是本条要禁的假成功。

要接着做完还缺一个可测的注入点：`Runtime` 内部固定构造
`RsyncSource<StdCommandRunner>`，而 replica 语义要求 `standby_master_addr` 非空 →
只能走真 ssh，本机/CI 无法集成测试。可选解法（需要定一个）：

1. 给 `RuntimeConfig` 加「本地 replica 来源」开关：`standby_master_addr` 为空但
   `standby_master_store_paths` 非空时按 `LocalDirSource` 建副本源（单机 standby 语义），
   集成测试即可用两个本地目录跑通 12b 的验收；
2. 或把来源做成泛型参数 / enum dispatch（`Source::Local | Source::Rsync`），
   避免 dyn 的同时允许测试注入；
3. 或在 `fkv_runtime` 暴露 `start_with_standby_source(...)` 的测试专用构造器。

另外还差：追赶成功后按表触发 `Request::Reopen`（需要把 shard sender 传进同步线程，
参照 `ArchiveCrond` 的 `stop_tx` + `join` 模式），以及 `Runtime::shutdown` 先停
standby 线程再停 shard 的顺序。

## C++ 实读（2026-11-04，`/Users/z/git/db/kv/eloqstore` @ 82b774c）

之前「缺可注入来源、只能真 ssh」的判断**是错的**，C++ 原文给出答案：

1. **单机 replica 就是 C++ 的测试形态**：`standby_master_addr = "local"` 是哨兵值。
   `DeriveStoreMode`（`include/eloq_store.h:122-134`）：`enable_local_standby` 为真时，
   `standby_master_addr` 空 → `StandbyMaster`，非空（含 `"local"`）→ `StandbyReplica`。
   `StandbyService::RemoteSpec`（`src/standby_service.cpp:638-659`）：
   `remote_addr` 空或 `== "local"` → 不加 `addr:` 前缀，直接本机 rsync。
   C++ 的 standby 集成测试（`tests/standby.cpp:514`）就是这么跑双 store 的：
   master `store_path=master_dir` + replica `standby_master_addr="local"` +
   `standby_master_store_paths={master_dir}`，用**真 rsync**（本机已装）。
   → Rust 的 `RsyncShell` 已经支持「空 ssh_addr 走本机 rsync」，缺的只是 Runtime 把
   `"local"` 归一到「本机来源」；不需要 dyn、不需要注入点。
2. **触发点在「每张表的 Reopen」，不是全局轮询**：`src/tasks/reopen_task.cpp:15-50`
   在 `StandbyReplica` 下 `standby_service->RsyncPartition(tbl_id, tag)` 后
   `WaitIo()` 等同步完成，再继续 reopen 读本地（刚装好的）manifest；
   `NotFound`/`ResourceMissing` 容忍 → `DropLocalManifest` + 清内存态（不写空快照）。
   `Runtime::reopen`（`crate/engine/fkv_runtime/src/lib.rs:643`）与 shard 的
   `Request::Reopen` 就是 Rust 侧对应的挂点。
3. **同步引擎是异步作业监督线程**：`StandbyService` = 单 supervisor 线程 +
   epoll/eventfd + pidfd 收割 rsync/ssh 子进程（`Start`/`SupervisorLoop`），
   作业从各 shard `SubmitJob`，`standby_max_concurrency` 限并发（默认 1）。
4. **`ListRemotePartitions` 是观察接口**：`shard.cpp` 的列举请求在用它，
   对应 metrics 的 `list_standby_partition`；**不是** replica 追赶的驱动。
   → 我落的 `RemoteSource::list_table_li` / `list_table_li()` 正好是它的 Rust 等价物；
   `StandbyService::catch_up()`（发现全部表并把 master 拉一遍）**超出 C++ 语义**，
   应改定位成管理/观察用途，不接进常规路径。
5. 数据同步命令：`rsync -a --inplace --include=data_* --exclude=* <remote_partition>/ <dest>/`
   （`StartRsyncJob`，:671-714）；manifest 单独由 `PrepareManifestJob` 装（term fencing
   在 `RunPrepareManifestJob`，cld-44 已覆盖其 Rust 版）；`archive_tag` 非空时同步的是
   该 tag 的 archive manifest（`RemoteManifestPath`，:616-637）。

## 修正后的实施计划（待做）

1. `backend_of`：standby 两种模式 → `Backend::Local`，并让 `Runtime` 持有
   `Option<Arc<StandbyService<RsyncSource<StdCommandRunner>>>>`（具体类型，不用 dyn）；
   `standby_master_addr == "local"`（或空但给了 `standby_master_store_paths`）→
   `RsyncSource::new(StdCommandRunner, master_path, "")`，否则用 ssh 前缀。
2. 把服务注入到 shard 的 reopen 路径（`ShardConfig` 加 `Option<Arc<...>>` 字段，
   `Request::Reopen` 处理处先 `sync_table(tbl)` 再走 manifest 装载），对齐
   `ReopenTask::Reopen`；`NotFound/ResourceMissing` → `DropLocalManifest` 语义
   （Rust 侧对应 store 的 drop-local-manifest 路径，需 fkv_store owner 配合）。
3. `Runtime::shutdown` 先停 standby 再停 shard；启动失败必须冒泡。
4. 集成测试 `fkv_runtime/tests/standby.rs`：两个本地目录 + `"local"` + 真 rsync，
   master 写入 → replica reopen 追到 → replica 可读；term 落后时拒绝安装。
5. 与 C++ 的已知差异（先记档）：Rust 尚无「单 supervisor 线程 + pidfd 收割」的并发上限，
   首版用 per-reopen 内联同步（`block()` 落阻塞线程池）替代，`standby_max_concurrency`
   的全局上限待补。

## DONE（2026-11-04，Runtime 接线完成，对齐 C++）

按上面「C++ 实读」的口径把 replica 接进 Runtime：

1. **`fkv_standby`**
   - `StandbyConfig::source_addr()`：`standby_master_addr` 为空或 `"local"` → `""`
     （本机 rsync），对齐 C++ `RemoteSpec` 的哨兵；
   - `sync_table_with_term(tbl, target_term)`：fencing term 由调用方给（对齐 C++
     `ReopenTask` 用 `store_->Term()`），`sync_table` 退化为用配置里的 term；
   - `RsyncStandbyService = StandbyService<RsyncSource<StdCommandRunner>>`（具体类型，无 dyn）；
   - 表发现：`list_table_li(root)` / `parse_table_name` / `table_li_of_name_li`
     （`LocalDirSource` 列举根目录，`RsyncSource` 一次 `ls -1`），即 C++
     `ListRemotePartitions` 的等价物；`StandbyService::catch_up()` 供管理/观察用途。
2. **`fkv_shard`**：`ShardConfig.standby: Option<Arc<RsyncStandbyService>>`；
   `Request::Reopen` 先走 `sync_standby_partition()`（对齐 C++ `ReopenTask::Reopen`
   的 `RsyncPartition` + `WaitIo`），
   - 同步成功后 **`page_io().evict_table(tbl)`** 掉旧的 FD 缓存/路径索引——sync 用
     rename 替换 data/segment/manifest 文件，不 evict 就会继续读被替换掉的旧 inode，
     表现为随后的 reopen 报 `Corrupted`（就是本轮踩到的坑）；
   - 远端分区不存在（`NotFound`/`ResourceMissing`）→ 只掉本地 manifest
     （`ManifestIo::drop_manifest`，绝不删远端），返回 `ResourceMissing` 交给
     reopen 延迟队列重试。
3. **`fkv_runtime`**：`backend_of` 对 standby 两模式返回 `Backend::Local`
   （standby 是「本地存储之上的复制」）；`standby_service()` 用
   `standby_master_store_paths[0]` + `source_addr()` 建服务并注入每个 shard；
   replica 缺 master 路径 → 启动即 `InvalidArgs`（不假装成功）。

验证（`crate/engine/fkv_runtime/tests/standby.rs`，真 rsync + 两个本地目录，
形态与 C++ `tests/standby.cpp` 一致）：
- `replica_catches_up_master_on_reopen`：master 写入 → replica `reopen` 即追上并可读；
  本地副本与 master 文件逐字节一致（`assert_same_files`）；master 再写一个 key →
  replica 再 reopen 增量追上；**master 停掉后 replica 读已同步数据不丢**；
- `replica_with_missing_master_fails_reopen`：master 目录缺失 → `ResourceMissing`
  （不是静默成功）；
- `fkv_runtime::modes standby_modes_start_and_shut_down` /
  `standby_replica_requires_master_paths`；
- `./test.sh` 467 passed / 1 skipped；`cargo clippy --workspace --all-targets
  --all-features --no-deps -- -D warnings` EXIT=0。

### 与 C++ 的有意偏离（记档，便于后续对齐）

1. **没有独立 supervisor 线程 / pidfd 收割**：C++ 是「单线程 + epoll/eventfd + 子进程
   pidfd」的异步作业引擎，`standby_max_concurrency` 限全局并发；Rust 首版在 reopen
   路径内联同步（`fkv_standby::block()` 落 blocking 线程池，不占 shard poll 栈），
   没有全局并发上限。单表 reopen 已经是天然的串行点；跨表并发上限待补
   （可复用同一 blocking 池 + 信号量，不需要新线程）。
2. **term fencing 在本地布局下是空转**：Rust 的 main manifest 名固定为
   `manifest_main_0`（cld-44 已裁定的偏离），`check_manifest_term` 读不到 term，
   所以「replica 以更小 term 拒绝安装」这条验收在单机形态下无对象；跨实现
   （C++ master）场景才可观测。
3. **远端分区消失后的处置**：C++ 清掉内存态后以「空表」继续返回成功；Rust 掉本地
   manifest 后返回 `ResourceMissing`，交给 reopen 延迟队列重试（最终失败）。要完全
   对齐需要在 `fkv_store` 加「清除该表内存态并以空表继续」的入口。
4. **`Reopen` 不带 archive tag**：C++ `RsyncPartition(tbl, tag)` 支持同步某个 tag 的
   archive manifest；Rust `Request::Reopen { tbl }` 没有 tag，只能同步当前 active
   manifest。

## 复审（2026-11-05 r6）

**通过。** DONE 声称的三层接线均在源码核实：

- `fkv_runtime/src/lib.rs:940-964` `standby_service()`：replica 缺 `standby_master_store_paths[0]` 即 `InvalidArgs`；`source_addr()` 为空/local 走本机 rsync。`fkv_shard/src/lib.rs:143` `ShardConfig.standby: Option<Arc<RsyncStandbyService>>`。
- `fkv_shard/src/lib.rs:1406` `sync_standby_partition`：`Request::Reopen` 先追分区（对齐 C++ ReopenTask），缺远端 → `drop_manifest` + `ResourceMissing`；两个 crate 的 `Cargo.toml:17` 均有 `fkv_standby` 生产依赖。
- 测试 `fkv_runtime/tests/standby.rs`：:96 `replica_catches_up_master_on_reopen`（:125 `assert_same_files` 逐字节比对 master/replica）、:152 `replica_with_missing_master_fails_reopen`；`tests/modes.rs:93/118` standby 启停与缺路径拒绝。
- 「与 C++ 的有意偏离」四条（无 supervisor 线程、term fencing 本地空转、远端消失处置、Reopen 无 tag）已记档，其中 tag 重开归 12a（todo），不判回归。
