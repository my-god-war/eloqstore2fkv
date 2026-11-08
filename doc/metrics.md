# 指标契约（fkv ↔ C++ eloqstore）

口径：**fkv 的指标是 C++ 的超集**，命名不逐个对齐，但要能对账。
本文件记录映射关系、桶口径来源、以及缺的上报通道，避免后续把「形似」当「已对齐」。

核对基线：eloqstore 仓 `82b774c`（2026-10-26）。`fkv/cpp/eloqstore` 是空目录，
C++ 侧事实取自独立 checkout：

- `include/eloqstore_metrics.h:24-56`：10 个指标名 + `ELOQSTORE_GAUGE_COLLECTION_INTERVAL = 1000`，整体在 `#ifdef ELOQSTORE_WITH_TXSERVICE` 内。
- `include/eloq_store.h:40-117`：`RequestType` 枚举（23 个）与 `RequestTypeToString`。
- `src/eloq_store.cpp:2626-2714`：`InitializeMetrics` 注册 19 个 `request_type` 标签。
- `src/storage/shard.cpp:220-246, 1506-1522, 1606-1642`：gauge / 请求指标采集点。
- `src/async_io_manager.cpp:1414-1424, 4208-4216`、`src/storage/page_manager.cpp:152-160`：gauge 数据源。

## 1. Op / request_type 映射

代码里的唯一事实源是 `Op::cpp_request_type_li()` 与
`Op::from_cpp_request_type()`（`crate/foundation/fkv_metrics/src/lib.rs`），
标签全集是 `CPP_METRIC_REQUEST_TYPE_LI`（19 个）。测试
`op_to_cpp_request_type_mapping` / `cpp_metric_tags_are_covered_once` 钉死映射。

| fkv `Op` | C++ `request_type` | 备注 |
|---|---|---|
| `Put` | `batch_write` | 名字不同，语义对应 |
| `Get` | `read` | 名字不同，语义对应 |
| `Scan` | `scan` | |
| `Reopen` | `reopen` | `ReopenArchive` 共用此 `Op`（tag 只是请求参数） |
| `FileGc` | `file_gc` | 一对一（C++ `FileGcRequest`） |
| `LocalGc` | `local_gc` | 一对一（C++ `LocalGcRequest`） |
| `Compact` | `compact` | |
| `Truncate` | `truncate` | |
| `Floor` | `floor` | |
| `Drop` | `drop` + `drop_table` | fkv 合并；对账时 2 个标签求和 |
| `CreateArchive` | `archive` | 名字不同；`DeleteArchive` 共用此 `Op`（C++ 同一个 `RequestType::Archive`） |
| `CreateBranch` | `create_branch` | |
| `DeleteBranch` | `delete_branch` | |
| `CleanExpired` | `clean_expired` | |
| `AttachShard` | `attach_shard` | |
| `DetachShard` | `detach_shard` | |
| `Prewarm` | （无） | fkv 自有请求（C++ 预热由 `PrewarmService` 后台触发，不是请求） |
| `ListArchiveTags` | （无） | fkv 自有请求（C++ 只有未注册指标的 `global_list_archive_tags`） |
| `ListObject` | `list_object` | |
| `ListStandbyPartition` | `list_standby_partition` | |
| `Shutdown` | （无） | fkv 自有请求，C++ 无标签 |

补充事实：

- C++ `RequestType` 枚举另有 `global_archive` / `global_reopen` /
  `global_list_archive_tags` / `global_create_branch`，但 `InitializeMetrics`
  **没有注册**这些标签，不出指标；`from_cpp_request_type` 对它们返回 `None`。
- C++ 只在该请求 `request_completed` 时记 `requests_completed` 与
  `request_latency`（重试 / 未完成不计），Rust 的 `observe` 在 shard 请求完成时
  调用，口径相同。
- 对账规则：先按上表把 C++ 标签求和（**只有 `Drop` 需要合并 `drop` +
  `drop_table`**），再与 fkv `Op` 比较；`Shutdown` / `Prewarm` /
  `ListArchiveTags` 没有 C++ 标签，单独列出，不参与对比。
- 上表与 `op_enum!` 宏一一对应，共 21 个 `Op`（`Op::ALL.len() == 21`，
  `fkv_metrics/tests/metrics.rs` 钉死）；`FileGc` / `LocalGc` 已拆分，
  不再有「合并求和」的 GC 行。

## 2. 延迟直方图桶

- fkv：`BUCKET_UPPER_US` = `[50, 100, 250, 500, 1k, 2.5k, 5k, 10k, 25k, 50k,
  100k, 250k, 500k, 1M, 2.5M, 5M]`（微秒）+ `+Inf` = 17 桶，由 `BUCKET_NUM`
  派生。**这是 fkv 自定义口径，不与 C++ 对齐。**
- C++：`eloqstore_metrics.h` 只注册 `Type::Histogram` 与 `request_type` 标签；
  桶边界在外部 txservice 的 `metrics::Meter`。核对过的 eloqstore checkout 内
  没有 `meter.h` / `metrics.h`，也没有任何桶边界常量，因此**无法在本仓证明
  「17 桶与 C++ 同桶」**。
- `todo.md` D38 的「17 桶延迟直方图」应读作 fkv 自定义口径；不要对外声称与
  C++ 对齐。跨语言比 p99 / tp99 前必须先统一桶边界，否则不可比
  （验收见 `todo.md` §7.6）。`fkv_bench` 的分位统计另用细粒度直方图，不复用
  这 17 桶（见 `perf-72`）。
- 若后续要抄 C++ 侧桶：需要 txservice 的 `meter.h` 或等价基线文档作为出处，
  并在 `BUCKET_UPPER_US` 旁注明来源。

## 3. 已有指标（fkv，无 C++ 对应物也算加分项）

- 每 shard：21 个 `Op` × (`total`, `error`, 17 桶 count/sum/min/max)。
- IO 字节级：`bytes_read/written`、page / manifest / segment / file 读写计数、
  `file_removes`、`fsync`/`dir_fsync`（`MeteredIo` 包装全部 IO；C++ 在
  `IouringMgr` 内部另有统计，命名不统一）。
- 缓存：云 overlay 与本地 index cache 的 `hit/miss/evict` 计数。
- 写队列 gauge（`eng-33b`）：`write_queue_depth`（排队等待启动的写请求数，
  跨分区合计）、`write_running`（全局在跑写任务数）。
- 读侧 gauge / 直方图（`eng-32c`，读请求 spawn 骨架见 `eng-32b`）：
  - `read_in_flight`（gauge）：当前在飞读任务数，`ReadFlight` 在提交 / 回收后
    写入；`read_in_flight_max` 记录进程内峰值。上限由
    `MAX_READ_IN_FLIGHT = 128`（`fkv_shard` 公开常量，= 4 × `MAX_READ_BATCH`）
    截断，峰值不应超过它。只统计 spawn 的读任务；`GetLarge` 也是窗口内的
    读任务，计入该 gauge，`PutLarge` 是写任务，不计入；
  - `read_batch_size`（直方图）：`submit_read_li` 每次批量提交的读请求条数。
    桶边界 `READ_BATCH_UPPER = [1, 2, 4, 8, 16, 32]` + `+Inf`，是**计数口径**，
    不复用延迟直方图的微秒桶；读窗口内逐条准入的读不计入批大小；
  - `io_in_flight` / `io_in_flight_max`（gauge）：`MeteredIo` 对每个异步 IO
    进出计数得到的全 shard 在飞 IO 数与峰值（读、写、manifest、file、segment
    都计入）。单读任务独占时即该请求的 IO 深度；多请求并发时用
    `read_in_flight` 均摊估算每请求平均深度。
- 读写口径一致：写侧复用 `eng-33b` 的 `write_queue_depth` / `write_running`，
  读侧新增字段与写侧一样是「shard 线程写、Runtime 线程 `metrics_snapshot` 读」。
- 调度面（`eng-34c`，双通道高低优先 + 时间片的可观测性）：
  - `yield_total`（计数）：时间片到点且低优先通道有货，把一条低优先插到高优先
    前执行的次数 —— 「后台确实被放行过」；
  - `round_over_budget_total`（计数）：主循环里连续服务高优先已超过
    `max_processing_time_us` 的轮数，**低优先通道为空时也会加** ——
    「前台确实霸占过时间片」。两个计数分开才分得清「后台没被饿」与「没人可放行」；
  - `low_pq_depth`（gauge）：低优先通道排队深度，入队侧 +1、shard 取走侧 -1
    （饱和），用于看后台积压是否被持续消化。

## 4. C++ 运行健康指标 ↔ fkv 对应

C++ 侧分两种采集节奏（`src/storage/shard.cpp`）：

- 每个非空闲轮：`work_one_round_duration`（Histogram）、`task_manager_active_tasks`（Gauge）；
- 每 1000 个非空闲轮：`index_buffer_pool_used`、`open_file_count`、
  `local_space_used`；
- 启动时一次：`index_buffer_pool_limit`、`open_file_limit`、`local_space_limit`。

| C++ 指标 | 类型 | fkv 对应 | 状态 |
|---|---|---|---|
| `index_buffer_pool_used` | Gauge | `ShardSnapshot::index_cache_bytes` | **已接**：`fkv_shard::sync_cache_metrics` → `ShardMetric::set_index_cache_bytes`；limit 未接 |
| `local_space_used` | Gauge | `ShardSnapshot::cloud_cache_bytes` | **已接**：`CloudStore::sync_cache_bytes` → `set_cache_bytes`；limit 未接 |
| `open_file_count` | Gauge | `CompioStore::fd_cache_len()` | 数据源已有；`fkv_io` trait 未暴露 fd 计数，尚未进 `fkv_metrics` |
| `open_file_limit` | Gauge | `CompioStore::fd_cache_limit()` | limit getter 已补；上报通道同上，未接 |
| `index_buffer_pool_limit` | Gauge | `fkv_config` 的 `index_cache_bytes` / 全 store 聚合预算 | 配置值存在，无法在 `fkv_metrics` 侧取到，未接 |
| `local_space_limit` | Gauge | `CloudStore::cache_budget()`（来自 `fkv_config::local_space_limit`） | 数据源已有，未进 `fkv_metrics` |
| `work_one_round_duration` | Histogram | 无 | `fkv_shard` 主循环未埋点 |
| `task_manager_active_tasks` | Gauge | `write_running` + `read_in_flight` | **部分覆盖**：C++ 取 `task_mgr_.NumActive()`，读写任务合计；fkv 读写分列，另补写队列深度、读峰值与读批直方图（`eng-32c` / `eng-33b`） |

说明：

- `ShardSnapshot::cache_bytes` 保留为
  `cloud_cache_bytes + index_cache_bytes` 的合计（兼容字段）；跨语言对账请用
  两个分项，否则会把 C++ 的两个 gauge 混成一个。
- 为什么不继续在 `fkv_metrics` 里加 `open_file_count` / `open_file_limit` 字段：
  gauge 必须有写入口，写入点在 `fkv_shard` / IO 装配处（不在本 issue 改动范围）。
  先加无写入点的 pub 字段等于死代码。解锁条件：`fkv_io` 增加 fd 计数取数通道
  （例如 `Store` 暴露 `fd_usage()`），或 `MeteredIo` 拿到可选的 fd 采样器；
  届时在 `sync_cache_metrics` 旁调用 `set_open_file_stat(count, limit)`。
- `index_buffer_pool_limit` 的数值来源是 `ShardConfig` 的
  `index_cache_bytes` / `index_cache_total_bytes`（取较小者生效），但
  `ShardMetric` 没有对应 setter，`fkv_shard` 也尚未调用。

## 5. 形态差异

C++ 全部指标在 `#ifdef ELOQSTORE_WITH_TXSERVICE` 内才编译，且每 shard 一个
`metrics::Meter`（标签 `shard_id`）；Rust 无条件编译，`ShardMetric` 也是每
shard 一个实例。对账时先确认 C++ 构建开了 `ELOQSTORE_WITH_TXSERVICE`，否则
C++ 侧根本没有这些指标。

## 6. bench 负载形状与稳态时间窗

`fkv_bench` 的延迟分位与吞吐来自端到端 workload（`crate/perf/fkv_bench`），
口径由 CLI 参数决定。对齐基线是 C++ `benchmark/simple_bench.cpp`（eloqstore
`82b774c`）。

### 6.1 写入

- 每个分区一条**独立写流水**：完成一个 put 立刻发本分区下一批，分区之间没有
  轮间 barrier（对齐 C++ `WriteLoop` 的 `finished` 队列收割即补发）。
- `--batch-size N` 是**单分区单请求**条数（对齐 C++ `batch_size`）。旧口径把
  它当成一轮全部分区的合计，`--partitions > 1` 时单请求只有约
  `N / partitions` 条；`--partitions 1`（CI 与历史默认跑法）下两种语义恒等。
- `--write-batchs N` 是**全系统**总批数（对齐 C++ 同名 flag），写入条数 =
  `N × batch_size`，与 `--rows` / `--max-key` 解耦；超过分区键数时按分区回绕
  覆盖，配合 `--del-ratio` / `--key-interval` 复刻覆盖写 + tombstone churn。
  缺省 `0` = 把 `--rows` 写一遍（历史行为，每分区最后一批可能不足
  `batch_size` 条）。CI 不传该参数，写吞吐 gate 仍走单遍口径。预算由各分区
  按完成顺序动态认领，`N` 小于分区数时可能有分区一个 batch 都没认领到。
- `--key-interval` 是**均匀网格**（第 i 个 key = `i × interval`）；C++
  `simple_bench` 是逐条随机步进 `rand() % interval + 1`。同参数下键密度 /
  树形状不同，跨实现 p99 / 吞吐比值只在 `interval = 1` 时严格可比，其余按趋势
  参考（登记见 `COMPAT_BASELINE` 与 `issue/done/r8e-02-*`）。
- `--del-ratio N`：每 `N` 个写把 `N - 1` 步之前的 key 改成 delete，delete 目标
  落在**同一分区**（旧实现按全局下标回退，多分区时会跨分区删不存在的 key）。
- **分区语义有意偏离**：fkv 的分区是全局键空间的分片（`index % partitions`），
  C++ 的每个 `Writer` 持有完整 `[0, max_key)` 键空间（多份全量副本）。同参数下
  C++ 每分区数据量是 fkv 的 `partitions` 倍，树深 / 文件数 / cache 命中率不可比；
  跨实现严格对等需 `--partitions 1`，或按分片换算后再比。

### 6.2 读与 scan

- 点读：`--read-per-part × --partitions` 条链并发、随机 key（对齐 C++
  `ReadLoop` 的 `num_readers`）。
- scan：同为 `--read-per-part × --partitions` 条并发链（对齐 C++
  `ScanLoop` 的 `num_scanners`），每请求一页 256 条（C++ `SetPagination(256, 0)`）、
  **随机起点**。`--scan-bytes N` 限定请求窗口：`N = 0` 扫到表尾；否则
  `scan_kvs = max(1, N / value_size)`，`start ∈ [0, rows - scan_kvs)`、
  `end = start + scan_kvs`（与 C++ 同公式，含其取不到 `rows - scan_kvs` 本身的
  边界行为）。窗口坐标是全局 key 下标，key 字节 = `下标 × key_interval`。
- 指标口径：
  - `scan_kv_s` / `scan_page_s` / `scan_*_us`：**页请求**口径，与 C++
    `g_scan_latency` / `ScanRequest` 同一物理量（每请求 ≤256 条、随机起点）；
  - `fullscan_kv_s` / `fullscan_*_us`：顺序全表遍历（旧 `scan_kv_s` 口径），
    C++ 无对应物，仅供诊断，不可与 `scan_*` 混比。**2026-11 断代（rev-03）**：
    旧的 `scan_kv_s` 是全表遍历吞吐、旧的 `scan_page_*` 是顺序翻页延迟，本次
    改造后 `scan_*` 改指页请求、旧口径改名 `fullscan_*`，跨代性能记录
    不可直接连曲线。
- 固定次数模式（`--test-secs 0`）：读满每分区一遍；scan 每链只发 1 个页请求
  （C++ 没有对应的固定次数口径）。时间窗模式（默认 60s）下读链 / scan 链
  逐请求循环到窗口耗尽，吞吐 = 窗口内完成数 / 窗口耗时。
- 性能记录新增顶层字段 `write_batchs` / `scan_bytes`（旧记录缺字段时按 `0`
  反序列化）与既有 `test_secs` 一起记录本次口径；跨 Rust / C++ 比较前先确认
  两侧参数一致。

### 6.3 时间窗与 kv-size

- `--test-secs N`（默认 60，对齐 C++ `FLAGS_test_secs`）：读与 scan 在 N 秒
  窗口内持续执行，分位按窗口内全部样本计算。写阶段仍按 `--write-batchs` /
  `--rows` 次数驱动，不随窗口放大。
- `--test-secs 0`：读 / scan 回到固定次数口径，与引入该参数前的行为一致
  （scan 主指标除外，见 6.2）。
- 性能记录的顶层 `test_secs` 字段记录本次口径（旧记录缺字段时按 `0`
  反序列化，即固定次数）。窗口不同（例如 Rust 固定次数 vs C++ 60s 稳态）的
  分位比值不可比。
- `--kv-size N`（默认 128，对齐 C++ `FLAGS_kv_size`）：整对 KV 宽度（含 8B
  key），value 取 `N - 8`，与 C++ 同名参数同义，可直接照抄参数。
  **2026-11 断代（rev-01）**：此前的性能历史序列里 `kv_size` 字段是
  value 字节数（默认 64），同字段异义——旧记录 + 8B 才是 pair 宽度；且旧默认
  64（value 口径）与新默认 128（pair 口径）写放量不同，未显式传参的跨代
  记录不可直接比吞吐与 MB/s。
