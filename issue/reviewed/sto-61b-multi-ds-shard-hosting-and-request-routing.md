# sto-61b 单 Store 托管多个 data store shard：per-request ds_shard + partition_filter + detach fanout

## 来源

`issue/new/sto-61-store-lifecycle-no-state-machine-no-multi-shard-hosting-attach-detach-divergence.md`
的多 shard 托管部分。属 runtime / API 大改，本 issue 不硬做。

## 位置

- Rust: `crate/engine/fkv_runtime/src/lib.rs`（`Runtime::attach_shard` / `detach_shard`
  只按 `shard_id` 索引线程，一个 Shard 只持一个 branch/term）、
  `crate/engine/fkv_api/src/lib.rs`（`Request` 无 `ds_shard`）
- C++: `src/eloq_store.cpp:922-1058`（`AttachShard` 的 `ds_shard_mux_` +
  `partition_filter` 校验）、`1060-1145`（detach 向每个 worker shard fanout）、
  `include/eloq_store.h:160-168`（`KvRequest::SetDsShard`）

## 现状与差异

- C++ 基线能力：「一个 Store 托管多个 data store shard」，每个请求携带
  `DsShardId`，Cloud 模式用 `partition_filter` 划分分区归属，Local / Standby 只允许
  一个 shard。
- Rust：`Request` 无 `ds_shard`，路由只按 `partition` hash；`Runtime` 的 `shard_id`
  是 OS 线程下标，不是 ds_shard 槽位；detach 只作用于本线程 shard，无跨线程 fanout。
- 因此无法表达 C++ 的「同 Store 多 ds_shard」与「detach 排空所有 worker shard」。

## 方案

1. `fkv_api::Request` 增加 `ds_shard: Option<u32>`（或包一层 `Envelope` 元数据），
   默认 `None` = 当前单 shard 行为。
2. `Runtime` 增加 ds_shard 槽位表（`Vec<DsShardSlot>`：state + branch + term），
   `attach_shard` 在槽位表上做 C++ 同款校验：
   - 非 Cloud 且已有其它非 `Detached` 槽位 → `InvalidArgs`；
   - Cloud 托管多个时必须配置 `partition_filter`；
   - 槽位状态 `Detached → Attached`，失败复位。
3. 请求路由改用 `ds_shard` + `partition_filter`，与现有 partition hash 组合；
   `None` 时保持现状。
4. detach 向所有 worker shard 投递 `DetachShard{ds_shard}` 并 `join_all` 等待排空，
   失败只告警，最后槽位置 `Detached`。
5. `fkv_config::KvOption` 补 `partition_filter`（与 sto-57 的未迁移选项 registry 对齐）。

## 依赖 / 重叠

- `issue/todo/12c-global-fanout-and-cloud-followups.md`（跨 shard fanout 先例）
- `issue/todo/sto-57c-non-migrated-options-registry.md`（`partition_filter` 解析）
- `issue/todo/eng-39b-store-auto-reopen-dead-code.md`（重开与 ds_shard 归属）

## 验收

- [ ] Cloud 模式 attach 两个带互斥 `partition_filter` 的 ds_shard 成功；写各自分区可读。
- [ ] Local / Standby 模式 attach 第二个 ds_shard 返回 `InvalidArgs`。
- [ ] Cloud 多 shard 未配 `partition_filter` 返回 `InvalidArgs`。
- [ ] detach 一个 ds_shard 后，其分区在全部 worker shard 上不可读，其它 ds_shard 正常。
- [ ] `cargo nextest run -p fkv_runtime -p fkv_shard` 全绿，clippy 零警告。

## rev-40 合并（2026-11-05 r6）

来源：`issue/reject/rev-40-partition-filter-not-implemented.md`（处置：**非拒绝，合并
跟踪**——本 issue 方案 1-5 已实质覆盖 `partition_filter` 的落地场景，rev-40 的增量是
C++ 消费点全量清单，甄别时已逐处复核行号属实，并入为验收清单）。

`partition_filter(ds_shard_id, tbl_id)`（`kv_options.h:445`）在 C++ 的全部消费点
——接入时逐处对照，**缺一处即扇出越权**（对不属本 ds_shard 的分区发
CreateArchive / Drop / CreateBranch / Reopen）：

- `eloq_store.cpp:100-101`（`CollectLocalPartitions`：attach 扇出的本地分区枚举过滤）
- `eloq_store.cpp:1541-1550`（`HandleDropTableRequest`：`std::erase_if` 按过滤器裁
  drop 扇出集合）
- `eloq_store.cpp:1728-1729, 1780-1781`（`HandleGlobalArchiveRequest`：create /
  delete 两处逐分区过滤）
- `eloq_store.cpp:1958-1959`（`HandleGlobalReopenRequest`）
- `eloq_store.cpp:2304-2305, 2357-2358`（`HandleGlobalCreateBranchRequest` 两处）
- `shard.cpp:800-803`（`Shard::EvictDsShard`：detach 驱逐集合 = filter 划给被
  detach ds_shard 的分区；无 filter = 全部归本 shard）
- `prewarm_task.cpp:515-516`（`PrewarmService::PrewarmCloudCache` 预热目标过滤）

Rust 落点对照：方案 2 attach 校验、方案 3 请求路由、方案 4 detach fanout 的驱逐
集合（`EvictDsShard` 对应物）、`GlobalFanout::dispatch`（global drop / branch /
reopen）、`ArchiveCrond::tick` 目标枚举（archive create/delete）、`PrewarmState::plan`
（预热）。`fkv_config` INI 面保持「进程内回调，非配置文件项」，与 C++ 同
（`eloq_store.h:1003/1032` 是 detach 与 filter 的语义约定出处）。

补充验收：

- [ ] 上表消费点在 Rust 侧逐处接入 `partition_filter`，无遗漏（含 archive crond 与
  prewarm）。
- [ ] 未设 filter 时行为与现状等价（默认路由取模语义两侧已核对一致：
  `eloq_store.cpp:1020` vs `TableIdent::shard_index`）。

## DONE（2026-11-05，范围判定：宿主集成范围外）

**不做 1:1，登记为有意偏离**（`COMPAT_BASELINE`「范围与排除项」）：该能力面向
C++ 云上单进程托管多个租户 data store shard 的宿主，分区归属由外部
`partition_filter` 回调决定；它是宿主集成设施而非存储格式/引擎语义。fkv 的
`Runtime` 是嵌入式单 attach 身份入口，多租户以每进程/每 Runtime 一个身份实现，
隔离性由进程边界提供。

现状与 C++ 未设 filter 的默认路径一致：请求按 `partition_id % shard_num`
路由（`eloq_store.cpp:1020` 同构），`attach_shard` 校验单一 attach 身份与
branch/term fencing，`detach_shard` 清空本 shard 的全部分区状态。

若未来要支持单进程多 DS 托管，重开条件：需要 per-ds-shard 的
`PartitionStoreMap` / branch / term / auto-reopen / archive / prewarm 全部按
`ds_shard` 分桶，并在 `fkv_api` 增加 per-request `ds_shard` 元数据；工作量为
runtime/shard 大改，不能以当前单身份状态机局部补丁实现。验收清单保留在本文件
供重开时使用。

## 复审（2026-11-05 v）

通过，归档（范围判定：有意偏离，已登记）。特别核对项（路由 / 过滤器 / 测试）：

- **请求路由未按 `DsShardId`/`partition_filter` 落地——属有意范围外**：
  `fkv_api/src/lib.rs` 的 `Request` 全文 `grep ds_shard` == 0，无 per-request
  ds_shard 元数据；路由仍按 `TableIdent::shard_index`（`fkv_runtime/src/lib.rs`
  :493/:2030/:800/:807 等）= `partition_id % shard_num`，与 C++ 未设 filter 时
  的默认 `shards_[ds_shard_id % shards_.size()]`（`eloq_store.cpp:1020`）同构。
- **`partition_filter` 未实现**，按宿主集成范围外裁定：
  `fkv_config/src/lib.rs:11-13` 模块文档「未迁移字段」登记「不做 1:1（sto-61b
  裁定范围外）」。
- **基线登记齐全**：`COMPAT_BASELINE:20-26`「单进程托管多个 data store shard
  ……不做 1:1」条目（含重开条件指针）；`doc/migration.md:207` M4 行注明
  「单 Store 多 DS 托管为范围外有意偏离」。
- **测试**：无 DsShardId/过滤器路由用例（因功能未实现）；验收清单（含 rev-40
  合并的 C++ 消费点全量对照）保留在本 issue 供重开时使用——与「范围外关闭」
  一致。
- DONE 的「不做 1:1」判定与 `00-index.md` 的 sto-61b 归属一致。
