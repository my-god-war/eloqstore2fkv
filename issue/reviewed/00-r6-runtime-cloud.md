# r6 复审报告：runtime / shard / cloud / standby / facade 运行时与云

日期：2026-11-05。Lane crate：`fkv_runtime`、`fkv_shard`、`fkv_cloud`、
`fkv_standby`、`fkv_object`、`fkv_object_s3`、`facade/{fkv,fkv_sdk,fkv_cli,fkv_testkit}`。

## 任务 A：done 复核

**复核 36 条，通过 36 条，未过 0 条**（每条已追加 `## 复审（2026-11-05 r6）`
并移入 `issue/reviewed/`，证据均为本次亲读 file:line）。

分组：

- **12 系列（5）**：`12-runtime-modes-api`（backend.rs 枚举分派 + modes/backend 测试）、
  `12b`（standby 三层接线 + standby.rs 真	rsync 集成）、`12c-followups`
  （store_path LUT/roots.rs/store_path.rs/cloud_e2e.rs，热点文件新落）、
  `12c-global-fanout`（GlobalFanout/global_*/ArchiveTarget + 内联三测）、
  `12d`（复审型条目，结论仍成立）。
- **cfg-01**：节状态机 / 内联权重 / ilog2 取整 + 四条 ini 测试，断言实质覆盖。
- **cld 系列（12）**：`cld-44`（fencing）、`cld-45`（每分区一次 rsync + sync_all 并发）、
  `cld-47`（DiskFull 预判）、`cld-47a`（PrewarmCrond 后台化）、`cld-51/51a/51b`
  （LRU O(1) + 阈值批量 drain）、`cld-52`（root_object_key）、`cld-53`（SDK 拒非 Local）、
  `cld-54`（prewarm 中止）、`cld-55`（metrics Arc::ptr_eq 接线断言）、`cld-56`
  （跨 term manifest 选择/GC 候选）。
- **sto 系列（15）**：`sto-58`（derive_mode standby 优先）、`sto-61/61a/61c`
  （状态机 + Local term 决策 A）、`sto-62/62a/62b/62c`（取消令牌 / deadline+Busy /
  Drop 语义）、`sto-63/63a/63b`（L1-partial 口径）、`sto-64/64a`（ReopenExternal）、
  `sto-69`（branch 归一）、`sto-70`（manifest 落名/清旧/fsync）。
- **perf-81 / perf-81a**：SDK exists/put_at 族 + scan_kv 返回 expire_ts。
- **超出典型清单补入（2）**：`eng-61`（shard 启动握手 + Drop 告警）、
  `eng-32b`（读 spawn，并行会话本轮刚移入 done，已按热点口径复核通过）。

跳过（非本 lane）：`27b`（store 读路径内核，归 read/store lane）、
`io-29a-d`/`io-34`/`io-3x`/`fmt-*`/`eng-8x` 等按 io/store/foundation lane 归属处理
（部分已被其它 lane 同步移入 reviewed/）。

已知边界（不算未过，已在对应 md 记档）：sto-62b 的
`request_queue_timeout_us` KvOptions 键与生产默认 deadline 未接（dragon 复核已记录，
挂 eng-72 收敛范围）；cloud_e2e 真断言需 `EKV_S3_ENDPOINT` 环境，本轮只验到门控 skip。

## 任务 B：新立项（2 条）

| 文件 | 一句话 | 严重度 |
|---|---|---|
| `issue/new/cld-57-…` | `global_list_archive_tags` 不按 `(tag,term)` 去重（C++ `std::unique` 去重，eloq_store.cpp:2217-2221），且 fkv_runtime 注释与 eng-84 DONE 误述「与 C++ 一致：不去重」 | P3 |
| `issue/new/cld-58-…` | standby replica 只用 `standby_master_store_paths[0]`，`standby_master_store_path_lut` 零消费；多根 master 分区漏同步且被 `drop_manifest`（C++ RemotePartitionPath 按表 LUT 选根，standby_service.cpp:572-610） | P2 |

两条均已 grep done/todo/reject/new 去重，md 头部标注热点文件（fkv_runtime staged
在途），证据来自 11-04 已合入的稳定代码，非半途伪差异。

## 遗留风险（无法下结论的区域）

1. **另一会话在途（git staged）**：`fkv_runtime/src/lib.rs`、`fkv_shard/src/lib.rs`、
   `fkv_cloud/src/lib.rs` 及新增 `roots.rs`/`read_spawn.rs`/`cloud_e2e.rs`/
   `store_path.rs` 正被 12 项 todo（12a/12d/27b/27c/eng-32c/eng-33a/eng-34b/eng-84/
   sto-61b）热改；其中 `12d`（runtime-cloud 统一预算）、`eng-32c`、`sto-61b`、
   `12a`（Request::Prewarm/ListPartitions）全部落在本 lane crate 上，落地形态未定，
   本轮不对其做差异结论。
2. **S3 真环境断言**：cloud_e2e / s3_rustfs / prewarm e2e 的真断言需 rustfs/minio
   （`docker/docker-compose.yml`），本轮与历轮一致只验到 skip 路径。
3. **跨 C++ master 混部**：cld-44/sto-70 已记录 main manifest 命名差异的混部判定，
   01 oracle 落地前不可验证。

## 最终判定

**不完善。** 缺口：

1. cld-58（P2，standby 多根 master 漏同步 + 本地 manifest 误 drop，正确性）。
2. cld-57（P3，归档 tag 列举去重 + 对齐性误述，控制面语义）。

其余：lane 内 done 条目 36/36 复核通过；除上述两条及在途 todo（已有归属）外，
本轮未发现其它未被覆盖的差异。
