# fkv 迁移进度

> 本文件由 `cargo run -p fkv_ledger -- report --out doc/progress.md` 生成，请勿手工编辑。

- 生成时间（UTC）：2026-11-05T04:19:54Z
- 兼容基线：eloqstore main 82b774c62728b023b03c6e5e84e69416ffec9d74 2026-10-26T22:55:36+08:00 / 兼容等级：L1-partial（Local + Rust 仅 append 分配 + 无压缩；C++ 交叉读写待 V2/V4 证据） / 范围与排除项（`fkv_ledger sync` 前两行更新时保留本块）： / - 分配模式：Rust 恒用 AppendAllocator；C++ 默认 data_append_mode=false（PooledFilePages）不在对齐范围。 / 互操作验收限定 data_append_mode=true 的 store（sto-59）。 / - 压缩：验收默认 enable_compression=false；压缩路径已实现，但保留决策非逐字节等价 / （Rust 比较「varint 前缀 + 压缩体」，C++ 仅比较压缩体，1~5 字节边界可能不同）。 / - 恢复语义：撕裂尾覆盖（sto-51）与 GetMapper 分配器重建（sto-52）未等价，不纳入互操作保证。 / - branch + large value：C++ `CreateBranch`（82b774c）的 `Snapshot` 不传 segment / mapping，branch manifest 无 segment 段；Rust 为保留 large value 继承与跨分支 / segment file_id 区间隔离仍写该段，属有意差异，不纳入逐字节等价 / （见 issue/reviewed/02-cpp-create-branch-omits-segment-mapping.md）。 / - 用户态 buffer pool / 页级 pin / WaitingZone（首访 single-flight）/ swizzle / 直取快路径 / data page cache 均不做 1:1（有意偏离，rev-41）——index 侧由 / `IndexCache`（Clock+代际+CacheLedger）替代，data 侧由内核 page cache / （默认 buffered）/ 直读（`with_direct_io`）替代（io-21）； / `enable_data_page_cache` 为解析不生效的死旋钮（INI 面兼容保留，sto-57）。 / 判定与重开条件见 issue/reviewed/rev-41-mem-cached-page-buffer-pool-not-implemented.md。 / - 单进程托管多个 data store shard（per-request `ds_shard` / `partition_filter` / / detach fanout，sto-61b）不做 1:1：该能力面向 C++ 云上单进程多租户宿主， / 由 `EloqStore::AttachShard` 的外部宿主提供分区归属回调；fkv 的 `Runtime` 是 / 嵌入式单 attach 身份入口，多租户以每进程/每 Runtime 一个身份实现。请求路由 / 仍按 `partition_id % shard_num`，与 C++ 未设 filter 时的默认路由一致 / （`eloq_store.cpp:1020`）。判定与重开条件见 / issue/reviewed/sto-61b-multi-ds-shard-hosting-and-request-routing.md。 / - `GlobalRegisteredMemory` 定长段注册内存 slab / ABA 空闲栈 / `SetEvictFunc` / 驱逐联动（rev-44）不做 1:1：C++ 中它是可选宿主集成设施 / （`kv_options.h` 明示留空即关闭），生产代码无 `SetEvictFunc` 调用点；fkv 以 / per-shard 启动注册 + 调用方 `FixedBuffer` 动态注册覆盖注册 IO，放不下自动 / 回退 buffered IO。segment 读已按批并发 `READ_FIXED`（rev-44 计划 1）。 / 大 value 超过 fixed 缓冲时的瞬态内存上界差异见 / issue/reviewed/rev-44-global-registered-memory-segment-slab-absent.md。 / - `sdk_runtime`（`KVCacheManager` / `KVCacheWorker` 的共享内存 buffer pool、 / manager/worker IPC 与 worker 侧 attach 协议）不做 1:1：它是 C++ 侧把多个 / worker 进程接入同一 KVCache 的宿主运行时（`include/sdk_runtime.h`、 / `src/sdk_runtime/*`），不属于存储引擎数据面；fkv 以嵌入式单 attach / `Runtime` 提供服务，多进程 / 共享内存缓存由外部宿主实现（与下面 / sto-61b 的宿主边界同一原则）。 / - C++ 测试辅助层（`include/test_utils.h` / `src/test_utils.cpp`： / `MapVerifier` / `ConcurrencyTester` / `ManifestVerifier`）不迁移：它们只服务 / eloqstore 自身的 C++ 测试，fkv 的测试与 `fkv_testkit` 是独立实现，迁移这层 / 不会增加任何用户可见能力。 / - bthread `EloqModule` 宿主集成（`include/eloqstore_module.h` / / `src/eloqstore_module.cpp`，仅在 `ELOQ_MODULE_ENABLED` 下编译）不迁移： / 它把 eloqstore 挂到 eloq 的 bthread 调度器上，属于 C++ 宿主侧集成； / fkv 的 `Runtime` 不是 eloq 模块，嵌入方直接调用 Rust API。 / - manifest record framing 使用 OS 页大小（C++ `page_align = sysconf(_SC_PAGESIZE)`）： / Linux 4 KiB、Apple Silicon 16 KiB。跨页大小主机的 manifest 互读不在保证范围 / （C++ 自身也不跨页大小兼容）；双向验收需在同一页大小主机执行。 / - data page 四分页重分配的切分点：C++ 在 restart region 边界切分，Rust 按 entry / 边界选择字节中点切分。两者生成的页都满足格式与互相可读，但同一输入不保证 / 逐字节相同；不影响崩溃恢复与 V2/V4 的读取兼容判定。 / - `fkv_sdk::KvStore` 为单根便捷入口：`store_path` 多根配置显式报 `InvalidArgs`， / 多根 / 多 shard 必须用 `fkv` facade 的 `start_runtime`（`RootedStore`）。 / 这是 API 形态差异，不是数据面能力缺失；SDK 首次访问会自动 reopen 已有 manifest。 / - S3 客户端并发模型：C++ 全部 shard 共用一个 `AsyncHttpManager` 线程池；fkv 每个 / shard 各持一个 `S3ObjectStore`（独立 tokio 线程 + 命令队列），运行时侧另有 / 全局列举（长驻 1 个）、预热 crond 与归档 crond（按配置启用）。`CloudStore` 含 / `RefCell` 不能跨线程共享，常态实例数是 / `shard_num + 1 + 已启用 crond 数`，由 `Runtime` 按真实构造点写入 / `CloudBackend::s3_instance_num`。该值是**稳态值**：并发的全局操作（同时进来的 / `global_archive` / `global_reopen` / 全局列举）在列举 store 已被占用时会各自 / 临时新建一个列举实例、用完归还即丢弃，瞬时总数可达 / `shard_num + 1 + crond 数 + 并发列举数`，而 `max_cloud_concurrency` 的均分 / 基数仍是稳态值，这段时间整机飞行请求可能超过配置额度（并发列举数受调用方 / 并发度约束，控制面操作不频繁）。`max_cloud_concurrency` 非 0 时按稳态实例数 / 均分（`CloudBackend::s3_config`）；均分向下取整且每实例至少 1，配置值小于实例数 / 时整机飞行请求仍可能超过配置值；为 0 时各实例回落环境 / 默认值（不做拆分）。 / 跨实例共享令牌池未实现，属已知资源模型差异，不影响对象命名与数据面互操作。 / - branch file id 预留：Rust `create_branch` 把父 allocator 高水位推到下一个 / 1024 file id 边界（`BRANCH_FILE_RESERVE_FILE_NUM`），C++ `CreateBranch` 不预留。 / manifest 自带 `file_range_li` 区间，双向读取不受影响；同 workload 的 file id / 布局不同，不纳入逐字节等价。 / - TTL 读过滤：Rust 读路径默认过滤 `expire_ts <= now` 的条目 / （`ttl_read_filter=true`），C++ `Read` / `Scan` / `Floor` 返回过期值由应用层 / 裁决。需要逐项对齐 oracle 时调用 `Store::with_ttl_read_filter(false)`。 / - `fkv_bench --key-interval` 是均匀网格（第 i 个 key = `i * interval`），C++ / `simple_bench` 是逐条随机步进 `rand() % interval + 1`。两者键分布 / 树形状 / 不同，跨实现 p99 与吞吐比值只能作趋势参考，不纳入数据面正确性或格式等价。 / - StandbyReplica 懒打开在 replay 前先 `sync_standby_partition` 追一次 master / （rev-46）：C++ 懒开路径只做本地 replay、依赖控制面 `Reopen`；Rust 副本重启 / / 换主后没有进程内收敛机制，因此在懒开入口补追赶。属**有意加强**，不改变盘面 / 格式；失败仍按 `ResourceMissing` 交给 auto-reopen 重试。 / - 以下 C++ 性能/资源旋钮在 Rust 侧解析但不生效（`KvOption::validate` 对非默认值 / 告警）：`enable_data_page_cache`、`init_page_count`、`coroutine_stack_size`、 / `max_archive_tasks`、`reserve_space_ratio`、`direct_io_buffer_pool_size`、 / `chunk_size`、`gc_global_mem_size_per_shard`、`pinned_tail_scratch_slots`、 / `cloud_request_threads`、`write_buffer_size`、`write_buffer_ratio`、 / `non_page_io_batch_size`、`segment_compact_yield_every`。完整清单与逐项决策见 / `fkv_config` 模块文档「未迁移字段」。`max_cloud_concurrency` / / `cloud_verify_ssl` / `allow_reuse_local_caches` / `cloud_auto_credentials` / 已接线。 / - 证据：format 层 7 类 22 个 case 已有真实双向语料——C++ 写 → Rust 读 / （`fixtures/cpp`）与 Rust 写 → C++ 读（`fixtures/rust`）；CI `fixture-diff` / 按 baseline 重新生成并 `git diff --exit-code fixtures/cpp` 检查漂移。 / 其中 `mapping` / `store` 的 payload 是**本仓定义的 format-envelope** / （见 `fixtures/README.md`），只证明 MappingSnapshot 值编码与目录命名 / 文件 / 布局解析一致，**不是**引擎级 store 目录互读。V2/V4（含续写 / 引擎级）仍未 / 系统化执行。 / L1 升为「已验证」前置：sto-51 修复 + V2/V4 执行 / （见 issue/reviewed/01-cpp-oracle-and-fixtures.md、issue/reviewed/sto-56、 / issue/reviewed/sto-59）。
- verified + ignored 覆盖率：12.5%

- 分流口径：todo 312 / ignored（范围外裁定）231 / <unmapped> 0；真实剩余（todo）= 312

## 审查看板（issue/）

真实开发进度的另一来源：`issue/` 目录按 run.md 流程流转。

| 目录 | 条目 |
|---|---:|
| todo | 0 |
| new | 11 |
| done | 5 |
| reviewed | 275 |
| reject | 10 |

## 状态总览

| status | count |
|---|---:|
| todo | 312 |
| claimed | 0 |
| in_progress | 0 |
| review | 0 |
| done | 1305 |
| verified | 0 |
| blocked | 0 |
| ignored | 231 |
| **total** | **1848** |

## 按 crate

| crate | total | todo | done | verified |
|---|---:|---:|---:|---:|
| fkv_api | 8 | 0 | 0 | 0 |
| fkv_btree | 13 | 10 | 3 | 0 |
| fkv_buf | 89 | 30 | 26 | 0 |
| fkv_cloud | 18 | 0 | 18 | 0 |
| fkv_codec | 27 | 14 | 13 | 0 |
| fkv_compress | 31 | 0 | 31 | 0 |
| fkv_config | 7 | 3 | 4 | 0 |
| fkv_error | 2 | 0 | 2 | 0 |
| fkv_failpoint | 8 | 8 | 0 | 0 |
| fkv_gc | 19 | 0 | 19 | 0 |
| fkv_io | 103 | 92 | 0 | 0 |
| fkv_io_compio | 234 | 0 | 234 | 0 |
| fkv_large | 21 | 0 | 21 | 0 |
| fkv_manifest | 9 | 0 | 9 | 0 |
| fkv_mapping | 131 | 21 | 97 | 0 |
| fkv_object | 73 | 2 | 71 | 0 |
| fkv_object_s3 | 65 | 0 | 65 | 0 |
| fkv_page | 121 | 22 | 99 | 0 |
| fkv_recovery | 8 | 0 | 8 | 0 |
| fkv_runtime | 13 | 13 | 0 | 0 |
| fkv_sdk | 86 | 0 | 0 | 0 |
| fkv_shard | 145 | 58 | 67 | 0 |
| fkv_standby | 51 | 2 | 49 | 0 |
| fkv_store | 241 | 0 | 241 | 0 |
| fkv_task_compact | 16 | 0 | 16 | 0 |
| fkv_task_prewarm | 24 | 0 | 24 | 0 |
| fkv_task_read | 5 | 0 | 5 | 0 |
| fkv_task_reopen | 3 | 0 | 3 | 0 |
| fkv_task_scan | 23 | 0 | 23 | 0 |
| fkv_task_write | 104 | 13 | 91 | 0 |
| fkv_testkit | 60 | 0 | 0 | 0 |
| fkv_types | 90 | 24 | 66 | 0 |

## 范围外裁定

共 231 条（status=ignored）；原因必须指向 `COMPAT_BASELINE` 的范围块。

| 原因 | 数量 |
|---|---:|
| eloq-module: bthread EloqModule 宿主集成不迁移（COMPAT_BASELINE 范围与排除项） | 8 |
| rev-41: 用户态 buffer pool / 页级 pin / data page cache 不做 1:1（COMPAT_BASELINE 范围与排除项） | 58 |
| rev-44: GlobalRegisteredMemory slab / tail scratch 不做 1:1（COMPAT_BASELINE 范围与排除项） | 11 |
| sdk-runtime: SDK 共享内存宿主运行时不做 1:1（COMPAT_BASELINE 范围与排除项） | 86 |
| sto-61b: 单进程多 DS shard 托管不做 1:1（COMPAT_BASELINE 范围与排除项） | 8 |
| test-utils: C++ 测试辅助层不迁移，fkv 用自身 testkit（COMPAT_BASELINE 范围与排除项） | 60 |

## 已变更需重验（0）

无。

## 下一批任务

| unit | crate | path |
|---|---|---|
| `include/async_io_manager.h#class_specifier:eloqstore::AsyncIoManager` | fkv_io | include/async_io_manager.h |
| `include/circular_queue.h#class_specifier:eloqstore::CircularQueue` | fkv_shard | include/circular_queue.h |
| `include/comparator.h#class_specifier:eloqstore::Comparator` | fkv_btree | include/comparator.h |
| `include/fail_point.h#class_specifier:eloqstore::FailPoint` | fkv_failpoint | include/fail_point.h |
| `include/pool.h#class_specifier:eloqstore::Pool` | fkv_buf | include/pool.h |
| `include/storage/page_manager.h#class_specifier:eloqstore::PageManager` | fkv_mapping | include/storage/page_manager.h |
| `include/tasks/archive_crond.h#class_specifier:eloqstore::ArchiveCrond` | fkv_runtime | include/tasks/archive_crond.h |
| `include/tasks/list_object_task.h#class_specifier:eloqstore::ListObjectTask` | fkv_object | include/tasks/list_object_task.h |
| `include/tasks/list_standby_partition_task.h#class_specifier:eloqstore::ListStandbyPartitionTask` | fkv_standby | include/tasks/list_standby_partition_task.h |
| `include/tasks/write_buffer_aggregator.h#class_specifier:eloqstore::WriteBufferAggregator` | fkv_task_write | include/tasks/write_buffer_aggregator.h |
| `include/utils.h#function:eloqstore::BuildStorePathListWithWeights` | fkv_types | include/utils.h |
| `src/coding.cpp#function:eloqstore::EncodeVarint32` | fkv_codec | src/coding.cpp |
| `src/kv_options.cpp#function:eloqstore::KvOptions::LoadFromIni` | fkv_config | src/kv_options.cpp |
| `src/storage/data_page_builder.cpp#function:eloqstore::ClampTimestampToInt64` | fkv_page | src/storage/data_page_builder.cpp |
