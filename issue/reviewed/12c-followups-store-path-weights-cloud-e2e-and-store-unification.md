# 12c 收尾遗留：store_path 权重、Cloud crond 端到端测试、运行时侧 CloudStore 统一

## 来源
`issue/done/12c-global-fanout-and-cloud-followups.md` 的未完成项（本轮只落了
fanout / batching / cache_budget / provider / Cloud 归档列举）。

## 1. `store_path` / `store_path_weights` / `store_path_lut` 未消费

现状：`KvOption::store_path` 只在 `validate` 里被 standby 段使用；`Runtime` 的
数据根目录来自 `RuntimeConfig::root`（facade 侧映射）。C++ 的多根目录 + 权重
（`FileManager` 按权重的分配表）在 Rust 侧无对应物，`data_path` 只有单根。

方案：`RuntimeConfig` 增加 `root_li: Vec<(PathBuf, u64)>`（或让 facade 从
`store_path`/`store_path_weights` 生成 LUT），`ShardConfig` 携带分配表，分区按
LUT 选根；`Runtime::start` 时校验权重长度 / 非 0（对齐 `validate` 的 standby 分支）。

验收：两路径等权时分区按 LUT 分布；权重 0 被拒；单根路径保持现行为。
重叠：`eng-72`（KvOption / Runtime / ShardConfig 定义重复）、`33-orphan-crates`。

## 2. Cloud 模式 crond / 全局请求的真 S3 端到端测试

现状：`cld-47a` 的 `s3_attach_triggers_prewarm` 已覆盖「attach → 后台预热 →
本地 cache 落文件」（需 `EKV_S3_ENDPOINT`）；但本轮新增的两条 Cloud 路径无
自动化验收：
- `ArchiveTarget::Cloud`：归档 crond 列**远端**分区（本地未 cache 的分区也要归档）。
  当前只有「运行时侧 CloudStore 能在非 compio 线程构造」的单测，列举 + 归档落盘
  没跑过。
- `Runtime::global_archive` / `Runtime::global_create_branch` /
  `Runtime::global_drop_table` 在 Cloud 下的目标集合（远端列举）；
  单分区回源用 `Runtime::reopen_external`（`global_reopen` 已删）。

方案：扩 `crate/engine/fkv_runtime/tests/prewarm.rs`（或新 `tests/cloud_e2e.rs`），
以 `EKV_S3_ENDPOINT` 门控：写远端 seed → 起 Runtime（cloud + archive_interval）→
断言远端出现归档 manifest；再跑一遍 `global_*` 断言回执条数与分区数一致。

依赖：本地 rustfs / minio（见 `docker/docker-compose.yml`）。

## 3. 运行时侧 `CloudStore` 实例统一（消除 2× 预算上界）

现状：shard 各自持一个 `CloudStore`，运行时侧的预热线程、归档 crond、
`global_*` 的远端列举各按需再建实例（装配参数一致，见
`open_runtime_cloud_store`）。`backend_of` 现在把 `local_space_limit` 接到
`cache_budget`（12c 第 3 项），两实例的 `resident_bytes` 互相看不见 →
极端情况下磁盘占用上界约 2× 预算（shard 不会误删预热落盘的文件，只是淘汰偏保守）。

方案（择一）：
- `fkv_shard` 暴露 cloud store 构造器 / 把 shard 的 `CloudStore` 通过通道服务化
  （`ListPartitions` / `Prewarm` 请求），运行时侧不再自建实例；
- 或 `fkv_api` 增加 `Request::Prewarm` / `Request::ListPartitions`，
  shard 自驱动（cld-47a 的原始设计）。
两种都改到 `fkv_shard/src/lib.rs` / `fkv_api`（并发热点），需单独排期。

验收：Cloud 模式下 runtime 侧不再持有与 shard 平行的 `CloudStore`；
cache 预算严格执行（evict 后 disk 占用 ≤ 预算）。

重叠：`issue/todo/12a-per-table-request-gaps.md`（`Request::Prewarm`）。

## DONE（2026-11-05）

第 1、2 项落地；第 3 项外置到
`issue/todo/12d-runtime-cloud-store-unified-budget.md`（含不硬做的理由）。

### 1. store_path / 权重 / LUT 选根

- `fkv_config`：
  - 新增 `MAX_STORE_PATH_LUT_ENTRIES = 1 << 20`、
    `compute_store_path_lut` / `compute_store_path_lut_capped`，算法对齐 C++
    `ComputeStorePathLut`（权重 0→1、gcd 归一、超上限等比缩放、槽位连续填充）。
  - `KvOption::validate` 对 `store_path` 无条件校验：权重个数必须与路径一致、
    权重 0 拒绝、空权重补等权 1；生成 `store_path_lut`。`standby_master_store_paths`
    同机制生成 `standby_master_store_path_lut`。
- `fkv_shard`：
  - 新增 `src/roots.rs`：`RootedConfig`（根列表 + LUT，构造期校验）与
    `RootedStore`（按 `partition_id % lut.len()` 选根，转发全部 IO trait）。
    单根 LUT = `[0]`，行为与单 `CompioStore` 一致；term 文件固定首根；
    多根时 `register_fixed_buffer` 返回 `InvalidArgs`，shard 已有回落路径
    （`put_large` / `get_large` 非 fixed），不会把 fixed 段 IO 写到错误根。
  - `Backend` 新增 `LocalRouted(RootedConfig)`；`StoreBackend::Local` 内部改为
    `RootedStore`（单根/多根同一实现），装配参数抽 `build_compio_store`。
  - 选择 Backend 变体而非给 `ShardConfig` 加字段：`ShardConfig` 字面量遍布 27
    处测试，新增字段会大面积打断并发热点；`backend` 字段本就属于
    `ShardConfig`，语义等价且零字面量改动。
- `fkv_runtime`：
  - `backend_of(options, root)`：`store_path` 非空时按 LUT 构造
    `Backend::LocalRouted`（单根也走路由，LUT `[0]`），空时保持
    `Backend::Local`；多根 standby 显式拒绝（rsync 追赶服务只有单
    `local_root`，多根会静默落后）。
  - `Runtime.root` → `Runtime.root_li`；Local 的归档 crond 目标与
    `global_list_archive_tags` 扫描全部根（Cloud 仍用 `RuntimeConfig::root`
    作为本地 cache）。
  - standby replica 的 `local_root` 取数据根（`store_path[0]`）。

### 2. Cloud 真 S3 e2e

- 新增 `crate/engine/fkv_runtime/tests/cloud_e2e.rs`，`EKV_S3_ENDPOINT` 门控，
  未设置时打印 `EKV_S3_ENDPOINT not set, skip cloud e2e test` 并返回。
- 远端 seed 两个分区（本地未 cache）→ 起 Runtime（cloud + `archive_interval_secs=1`）
  → attach → 轮询远端出现两个分区的归档 manifest（覆盖
  `ArchiveTarget::Cloud` 的远端列举 + 归档落盘）→ `global_archive` /
  `global_create_branch` / `global_drop_table` 回执条数 = 分区数，且
  `reopen_external` 返回 `Installed`（单分区回源）。
- 本机无 rustfs，只验证了门控 skip 路径；真 S3 断言需在
  `docker/docker-compose.yml` 的 `test` 服务里跑。

### 3. 运行时侧 CloudStore 统一

外置 `12d`：真正统一依赖 `Request::Prewarm` / `ListPartitions`（`fkv_api` +
`fkv_shard` + `fkv_metrics` 穷尽 match 的并发热点，且与 `12a` 同批次）；
“共享原子预算”不能严格约束磁盘（各实例只能淘汰自己的 LRU）。已在
`open_runtime_cloud_store` 文档写明当前边界，并确认归档/全局列举实例
`resident_bytes` 恒 0（只读远端），双重记账只在 prewarm 与 shard 实例之间。

### 测试

- `cargo test -p fkv_config --tests` → 21 passed。
- `cargo test -p fkv_shard --test roots` → 1 passed（LUT 选根、reopen 读回）。
- `cargo test -p fkv_shard --test backend/cancel/cloud_cache_metrics_wiring/
  cloud_reopen/deadline/index_cache_metrics/lifecycle/priority/
  queue_backpressure/read_batch/write_gate` → 全过。
  `oom_retry` 并行跑时 failpoint 全局计数互相干扰（单线程复跑通过），与本改无关。
- `cargo test -p fkv_runtime --tests` → 全过（`long_stress` 保持 ignored）；
  新增 `tests/store_path.rs` 4 项：等权双根分布、单根保持、权重 0/个数不符
  启动前拒绝、归档 crond + `global_list_archive_tags` 覆盖双根。
- `cargo test -p fkv_runtime --test cloud_e2e -- --nocapture` → 门控 skip。
- `cargo clippy -p fkv_config -p fkv_shard -p fkv_runtime --all-targets
  --no-deps -- -D warnings` → 0 警告。

### 已知限制

- Cloud 模式的本地 cache 仍只认 `RuntimeConfig::root`（`store_path` 多根只用于
  Local/Standby 数据面）。
- 多根 + `fixed_buffer_size > 0`：fixed buffer 注册被拒，shard 日志告警后回落
  非 fixed 大值路径（数据正确性不受影响，性能略降）。
- `fkv_runtime` doctest 在本机与并发的其它 agent 编译产物冲突
  （`can't find crate`）；`--tests` 全过，未改任何 doctest。


## 复审（2026-11-05 r6）

**通过（第 1、2 项）。** 热点文件：本轮 git status 显示 fkv_shard/fkv_runtime/fkv_cloud 仍在并行会话在途改动中，行号以本次实读为准。

- 第 1 项：`fkv_config/src/lib.rs:62` `MAX_STORE_PATH_LUT_ENTRIES=1<<20`、:69-95 `compute_store_path_lut[_capped]`（gcd 归一、超限等比缩放、槽位连续填充）；`fkv_shard/src/roots.rs:28/55/59` `RootedConfig/RootedStore::root_of`（`lut[partition_id % len]` 选根，转发全部 IO trait）；`backend.rs:65/74/81-89` `Backend::LocalRouted` + `StoreBackend::Local(Box<RootedStore>)`；`fkv_runtime/src/lib.rs:839 local_root_li`、:908-913 多根 standby 显式拒绝。
- 测试：`fkv_shard/tests/roots.rs:75` `local_routed_places_partition_on_lut_root`（:106-109 断言奇偶分区各落一个根）；`fkv_runtime/tests/store_path.rs:77/110/140/161` 四条（等权分布/单根不变/权重 0 与个数不符启动前拒绝/归档 crond+global_list_archive_tags 扫全部根）。
- 第 2 项：`fkv_runtime/tests/cloud_e2e.rs:145` `cloud_archive_crond_and_global_requests_hit_remote_targets`，:147 `EKV_S3_ENDPOINT` 未设即 skip，:193-209 覆盖 global_archive/create_branch/drop_table 回执条数与 reopen_external。本机无法验真 S3 断言（门控路径属实）。
- 第 3 项确认外置 `todo/12d-runtime-cloud-store-unified-budget.md`（现存），不算缺失。
