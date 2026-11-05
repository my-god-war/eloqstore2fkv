# sto-60 Metrics 表面与 C++ eloqstore_metrics.h 大面积错位；「17 桶对齐」不可验证

## 位置
- Rust: `crate/foundation/fkv_metrics/src/lib.rs:20-24`（`BUCKET_UPPER_US` 16 上界 + +Inf = 17 桶）、`28-47`（`Op` 16 变体）、`205-250`（`ShardMetrics` 计数器组）、`663` 行 `MeteredIo` 包全部 IO
- C++: `include/eloqstore_metrics.h:32-54`（仅 8 个命名指标，且整体包在 `#ifdef ELOQSTORE_WITH_TXSERVICE` 内）、`src/eloq_store.cpp:2626-2685`（`InitializeMetrics` 注册，latency/completed 按 `request_type` 标签切分 20 种）、`include/eloqstore_metrics.h:51-54`（gauge 采集间隔 `ELOQSTORE_GAUGE_COLLECTION_INTERVAL = 1000`）

## 现状
Rust `fkv_metrics`：每 shard 16 个 Op（put/get/scan/reopen/gc/compact/truncate/floor/drop/create_archive/create_branch/delete_branch/clean_expired/attach_shard/detach_shard/shutdown）× (total, error, 17 桶直方图 count/sum/min/max)；另有 bytes_read/written、page/manifest/segment/file 读写计数、file_removes、cache hit/miss/evict/bytes、fsync/dir_fsync。todo.md D38 称「17 桶延迟直方图」。

C++ `eloqstore_metrics.h` 只有 8 个命名指标：`eloqstore_work_one_round_duration`、`eloqstore_task_manager_active_tasks`、`eloqstore_request_latency`、`eloqstore_requests_completed`、`eloqstore_index_buffer_pool_used/limit`、`eloqstore_open_file_count/limit`、`eloqstore_local_space_used/limit`。latency/completed 按 `request_type` 标签展开（read/floor/scan/list_object/list_standby_partition/batch_write/reopen/truncate/drop/drop_table/archive/compact/local_gc/file_gc/clean_expired/create_branch/delete_branch/attach_shard/detach_shard，`eloq_store.cpp:2655-2676`）。**直方图桶定义不在本仓库**——挂在外部 txservice 的 `metrics::Meter`（`Register(..., Type::Histogram, {{"request_type", {...}}})` 只给标签，不给桶）。

## 差异与风险
- **Op 命名不对齐**：Rust `put/get` vs C++ `batch_write/read`；Rust `create_archive` vs C++ `archive`；Rust 有 `shutdown`（C++ 无）；Rust 缺 `list_object`、`list_standby_partition`、`drop_table`、`local_gc`、`file_gc`（Rust 用单一 `Gc` 合并，C++ 区分 file_gc/local_gc；drop 与 drop_table 合并）；跨语言对账指标 delta 时字段名不互通。
- **缺 C++ 的 gauge 族**：index_buffer_pool_used/limit、open_file_count/limit（fd 泄漏监控）、local_space_used/limit（磁盘预算）、work_one_round_duration、task_manager_active_tasks——Rust 全部没有；这些恰是 C++ 每 WorkOneRound 周期采集的运行健康指标（`eloqstore_metrics.h:51-54`）。
- **「17 桶对齐 C++」无法证实**：eloqstore 仓库内不存在桶定义；D38 的表述在账本/文档里给人「与 C++ 同桶」的预期，实际不可验证，验收口径（todo.md §7.6 性能对比）依赖延迟分布时两边桶边界不同则 p99 不可比。
- C++ 全部指标在 `ELOQSTORE_WITH_TXSERVICE` 宏下才编译（`eloqstore_metrics.h:24、56`），Rust 无条件编译——构建形态不同源。
- Rust 的 IO 字节级指标（MeteredIo）是 C++ 没有的超集（C++ 在 IouringMgr 内部另有统计），这部分是加分项，但命名体系未统一。

## 对齐建议
- 定义指标契约文档：Rust Op 名 ↔ C++ request_type 标签映射表（put→batch_write、get→read、create_archive→archive、drop→drop、Gc→file_gc/local_gc 拆分），缺的 C++ 标签明确标注「无对应请求」；
- 桶定义二选一：从 txservice `meter.h`（或等价基线文档）抄桶边界写进 `fkv_metrics` 并加注释出处，或改口「17 桶为 fkv 自定义口径」；
- 补 C++ gauge 族的 Rust 对应（open_file_count 需要 CompioStore fd LRU 暴露当前打开数；local_space_used 需要目录配额统计；index_buffer_pool 对应 IndexCache 字节数）；
- 若验收用 `fkv_bench` 对比 p99，两边需统一桶边界后再比。

## 性能优化建议
- 直方图桶定位已用 `position(|upper| ...)` 线性扫（`fkv_metrics/src/lib.rs:119-122`），16 桶可接受；若扩桶再改二分。

## 严重度
P2（性能/健壮性）

## 审查结论（2026-11-04）

**部分采纳**。事实成立：`Op` 命名与 C++ `request_type` 不对齐、缺 C++ gauge 族、
「17 桶对齐」在本仓不可证实（C++ 桶定义在外部 txservice）。

已落地（本地可完成部分）：

- 新增 `doc/metrics.md`（指标契约）：16 个 `Op` ↔ C++ `request_type` 标签映射表
  （`Put→batch_write`、`Get→read`、`CreateArchive→archive`、`Gc→file_gc+local_gc`、
  `Drop→drop+drop_table`、`Shutdown` 无对应、C++ 的 `list_object` /
  `list_standby_partition` fkv 无）；对账规则（合并标签求和后再比）；
  「17 桶是 fkv 自定义口径」的定论与 p99 跨语言可比性前提；缺的 5 个 gauge 及补法；
  C++ 指标在 `ELOQSTORE_WITH_TXSERVICE` 内才编译的形态差异。
- `fkv_metrics::BUCKET_UPPER_US` 补出处注释，指向 `doc/metrics.md`。
- 说明：`cpp/eloqstore` 为空，C++ 侧的指标名与行号取自 issue 摘录，未经源码复核，
  文档已标注该前提。

## 阻塞（第 4 节 gauge 族，跨 crate）

- `open_file_count/limit` 需 `fkv_io_compio` fd 缓存暴露当前打开数（见 `eng-79`）；
- `index_buffer_pool_used/limit` 需 IndexCache 上报字节数（见 `eng-66`/`eng-69`）；
- `local_space_used/limit` 归属未定（需要目录配额统计，owner 未定）；
- `work_one_round_duration` / `task_manager_active_tasks` 需 `fkv_shard` 事件循环与队列深度上报。

以上四条都不在本次可改范围（热点文件 / 未定 owner），故本条留在 todo/。

## 复核（2026-11-05）

结论：**保持 todo**，不写 DONE。第 1 项（契约文档 + 桶口径）已在首轮落地，
本轮把映射表补进代码定义处，第 2 项仍无安全写入点。

- 已落地：`fkv_metrics::Op` 的文档注释补齐 Op ↔ C++ `request_type` 映射表
  （`put→batch_write`、`get→read`、`create_archive→archive`、
  `gc→file_gc+local_gc`、`drop→drop+drop_table`、`shutdown` 无对应）。
- `doc/metrics.md` §4 按当前源码校正了各 gauge 的数据源现状：
  - `open_file_count`：`CompioStore::fd_cache_len()` 已有，`fd_limit` 只有 setter
    没有 getter；`fkv_io` trait 不暴露 fd 计数，`Store`/`MeteredIo` 缺取数通道，
    写入点还得落在 `fkv_shard` 主循环 —— 跨 3 个 crate，不在本次范围。
  - `local_space_used`：limit 已有（`fkv_config::local_space_limit`），used 无任何
    统计，需新增目录用量采集，owner 未定。
  - `index_buffer_pool_limit` / `work_one_round_duration` /
    `task_manager_active_tasks`：写入点均在 `fkv_shard`（并行改动区）。
- 不硬造 gauge 字段的理由：无写入点的 pub 字段 = 死代码，违反仓库 SKILL 的零死代码要求。
- 验证：`cargo check -p fkv_metrics`、`cargo nextest run -p fkv_metrics`（4 passed）、
  `cargo clippy -p fkv_metrics --no-deps --all-features --all-targets -- -D warnings` 全绿。

解锁条件：`fkv_io` 增加 fd 计数取数通道（或 `Store` 暴露 `fd_cache_len`）后，
可在 `sync_cache_metrics` 旁加 `set_open_file_count/limit`；届时再考虑拆出单独 todo。

## DONE（2026-11-05）

C++ 侧本次已用独立 checkout 复核（eloqstore `82b774c`，2026-10-26），不再依赖
issue 摘录；`fkv/cpp/eloqstore` 仍是空目录。

改动：

- `fkv_metrics` 新增显式映射 API（`crate/foundation/fkv_metrics/src/lib.rs`）：
  - `Op::cpp_request_type_li()`：`Put→batch_write`、`Get→read`、
    `CreateArchive→archive`、`Gc→[file_gc, local_gc]`、
    `Drop→[drop, drop_table]`、`Shutdown→[]`，其余同名；
  - `Op::from_cpp_request_type()`：标签反查 `Op`，`list_object` /
    `list_standby_partition` 与未注册指标的 `global_*` 返回 `None`；
  - `CPP_METRIC_REQUEST_TYPE_LI`：C++ `InitializeMetrics` 注册的 19 个标签全集
    （`eloq_store.cpp:2655-2712`）。映射由单一 `op_enum!` 表生成，不会漂移。
- 17 桶口径：
  - `BUCKET_UPPER_US` 注释改为「fkv 自定义口径，不与 C++ 对齐」，并注明
    eloqstore checkout 内没有 `meter.h` / `metrics.h` 及桶边界常量；
  - `doc/metrics.md` §2 同步定论：D38 的「17 桶」不可解读为与 C++ 同桶，
    跨语言 p99 前必须先统一桶边界。
- gauge 对应（有真实写入点的才接）：
  - `ShardSnapshot` 拆出 `cloud_cache_bytes`（C++ `local_space_used`，写入点
    `CloudStore::sync_cache_bytes`）与 `index_cache_bytes`（C++
    `index_buffer_pool_used`，写入点 `fkv_shard::sync_cache_metrics`）；
    `cache_bytes` 保留为两者之和（兼容）。
  - `fkv_io_compio` 增加 `CompioStore::fd_cache_limit()`（C++
    `IouringMgr::GetOpenFileLimit` 的数据源；`fd_cache_len()` 已存在对应
    `GetOpenFileCount`）。
- `doc/metrics.md` 按 C++ 源码重写：核对基线/行号、19 个注册标签、采集节奏
  （每轮 Histogram、每 1000 非空闲轮 used gauge、启动时一次 limit）、
  gauge 对应表与未接通道。

测试：

- `cargo test -p fkv_metrics`：7 passed（新增 `op_to_cpp_request_type_mapping`、
  `cpp_metric_tags_are_covered_once`、`cache_gauges_split_cloud_and_index`）。
- `cargo test -p fkv_io_compio`：全部通过（含 `hardening` 的 2 个 fd 上限断言，
  校验新 getter 的配置值与 `usize::MAX` 不限语义）。
- `cargo clippy -p fkv_metrics --all-targets --no-deps -- -D warnings` 通过；
  `cargo clippy -p fkv_io_compio --all-targets --no-deps -- -D warnings` 通过。
  为消 nightly 的 `Atomic::fetch_update` 弃用告警，把并发改动中的该调用改为
  CAS 循环（替代 API `try_update` 高于本仓 MSRV 1.89）。
- `cargo +nightly fmt -p fkv_metrics -- --check` 通过。

明确未做（保留在 doc/metrics.md §4）：

- `open_file_count/limit` 尚未进 `ShardMetrics`：数据源已有
  （`fd_cache_len` / `fd_cache_limit`），但 `fkv_io` trait 不暴露 fd 计数，
  写入点需落在 `fkv_shard` / `MeteredIo`，不在本 issue 改动范围；不硬造无写入点字段。
- `index_buffer_pool_limit` / `local_space_limit`：配置值已有，缺
  `ShardMetrics` setter 与 shard 调用点。
- `work_one_round_duration` / `task_manager_active_tasks`：需 `fkv_shard`
  主循环埋点；`write_queue_depth` / `write_running` 只是部分覆盖。

## 复审（2026-11-05 r6）

结论：**通过**（本轮范围 = 映射 API + 桶口径定论 + 有真实写入点的 gauge 拆分；未做项已在 §4 记录且理由成立）。

- 映射 API：`fkv_metrics/src/lib.rs:77` `cpp_request_type_li`、`:87-91` `from_cpp_request_type`、`:122` `CPP_METRIC_REQUEST_TYPE_LI`（19 标签，`op_enum!` 单表生成）。
- 桶口径：`:21` 注释「fkv 自定义口径，不与 C++ 对齐」。
- gauge 拆分：`:278` `index_cache_bytes`、`:471-472` snapshot 拆 `cloud_cache_bytes`/`index_cache_bytes`（和保留为 cache_bytes）；`fkv_io_compio/src/lib.rs:945` `fd_cache_limit()`。
- 测试 `fkv_metrics/tests/metrics.rs:71`（三条关键映射 + 拆分逐条钉死）、`:95`（19 标签覆盖恰一次）、`:121`（cloud/index 双 gauge 分读、和兼容）。断言真实覆盖。
