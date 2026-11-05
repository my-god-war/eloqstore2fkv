# 12d Runtime 接入 Standby（12b 收尾）

## 来源

`issue/done/12b-standby-runtime-wiring.md` 只落地了 replica 追赶**原语**
（表发现 + `catch_up` / `sync_all`），Runtime 接线被**有意保持未放开**，其
`## 阻塞（Runtime 接线仍差一步，故意不放开）` 与 `## 修正后的实施计划（待做）`
仍未完成。12b 归档后 `fkv_standby` 仍是孤儿 crate，`tool/orphan_allow.tsv`
的 owner 指向 12b 已失效，本 issue 承接该归口。

## 现状

- `fkv_runtime::backend_of` 对 `StoreMode::StandbyMaster` / `StandbyReplica` 仍返回
  `InvalidArgs`（未静默降级）。
- `crate/io/fkv_standby/src/lib.rs` 已具备 `StandbyService` / `catch_up` /
  `RemoteSource`（`LocalDirSource` / `RsyncSource`）。
- 任何 crate 的 `Cargo.toml` 都未依赖 `fkv_standby`（生产依赖图为空）。

## 待做（沿用 12b 修正后计划）

1. `backend_of`：standby 两种模式 → `Backend::Local`；`Runtime` 持有
   `Option<Arc<StandbyService<RsyncSource<StdCommandRunner>>>>`（具体类型，不用 dyn）。
   `standby_master_addr == "local"`（或空但给了 `standby_master_store_paths`）→
   本机来源，否则 ssh 前缀。
2. 把服务注入 shard 的 reopen 路径（对齐 C++ `ReopenTask::Reopen`：
   `RsyncPartition` → 等同步 → 再读本地 manifest；`NotFound`/`ResourceMissing` 容忍 →
   `DropLocalManifest` + 清内存态，不写空快照）。
3. `Runtime::shutdown` 先停 standby 再停 shard；启动失败必须冒泡。
4. 集成测试 `fkv_runtime/tests/standby.rs`：两个本地目录 + `"local"` + 真 rsync，
   master 写入 → replica reopen 追到 → replica 可读；term 落后时拒绝安装。
5. 已知差异记档：Rust 首版用 per-reopen 内联同步替代 C++ 的单 supervisor 线程 +
   pidfd 收割，`standby_max_concurrency` 限流范围相应变窄。

## 验收

- `backend_of` 不再对 standby 报 `InvalidArgs`；`cargo nextest run -p fkv_runtime` 通过。
- `fkv_standby` 进入生产依赖图后，从 `tool/orphan_allow.tsv` 删除该行（守卫会报过期）。
- `cargo clippy -p fkv_runtime --all-targets --no-deps -- -D warnings` 零警告。

## 归口

- 承接 `issue/done/12b-standby-runtime-wiring.md` 未完成部分；
- 与 `issue/new/sto-61-store-lifecycle-no-state-machine-no-multi-shard-hosting-attach-detach-divergence.md`
  （StoreMode 装配 / 生命周期状态机）重叠处以其为准；
- `fkv_standby` 的孤儿白名单 owner 指向本文件。

## 复审（2026-11-04）：本条前提已不成立，接线已由 12b 完成

逐条核对当前源码：

- 「`backend_of` 对 standby 仍返回 `InvalidArgs`」——**已过期**：
  `crate/engine/fkv_runtime/src/lib.rs` 的 `backend_of` 对
  `StoreMode::StandbyMaster | StandbyReplica` 返回 `Ok(Backend::Local)`。
- 「任何 crate 的 `Cargo.toml` 都未依赖 `fkv_standby`」——**已过期**：
  `fkv_shard/Cargo.toml`、`fkv_runtime/Cargo.toml` 都有生产依赖
  `fkv_standby = { path = "../../io/fkv_standby" }`。
- 「服务未注入 reopen 路径」——**已落地**：`ShardConfig.standby:
  Option<Arc<RsyncStandbyService>>`，`fkv_shard::sync_standby_partition` 在
  `Request::Reopen` 里先追该分区（对齐 C++ `ReopenTask::Reopen`），同步后
  `page_io().evict_table(tbl)` 掉旧 FD/路径缓存，远端分区缺失时
  `ManifestIo::drop_manifest` + `ResourceMissing`。
- 「集成测试缺」——**已有**：`crate/engine/fkv_runtime/tests/standby.rs`
  （`replica_catches_up_master_on_reopen` / `replica_with_missing_master_fails_reopen`，
  两个本地目录 + 真 rsync；含「master 停后 replica 读不丢」「增量追赶」）。
- 白名单归口：`tool/orphan_allow.tsv` 里 `fkv_standby` 那行已删除
  （crate 已有生产消费者），删后 `bash tool/orphan.sh` 仍 EXIT=0。

详见 `issue/done/12b-standby-runtime-wiring.md` 的 `## DONE` 与「与 C++ 的有意偏离」
（无 supervisor 线程/并发上限、本地布局下 term fencing 空转、远端分区消失的处置差异、
`Reopen` 不带 archive tag）。所有验证：`./test.sh` 467 passed，workspace clippy EXIT=0。

## 复审（2026-11-05 r6）

**通过（其 2026-11-04 复审结论与当前源码一致）。** 抽验：

- `fkv_runtime/src/lib.rs:872` 起 `backend_of` 对 StandbyMaster/StandbyReplica 走本地数据面 + `standby_service()`（:940）注入；`fkv_shard/Cargo.toml:17`、`fkv_runtime/Cargo.toml:17` 均含 `fkv_standby` 生产依赖。
- `fkv_shard/src/lib.rs:1406` `sync_standby_partition` 在 Reopen 路径生效；`fkv_runtime/tests/standby.rs:96/152` 两条集成测试在位。
- 本文件自身即复审记录，无独立开发量；随 12b 一并归档。
