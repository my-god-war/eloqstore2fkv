# 12d 运行时侧 CloudStore 与 shard 实例统一（cache 预算严格上界）

## 来源
`issue/done/12c-followups-store-path-weights-cloud-e2e-and-store-unification.md`
第 3 项（12c 只落了 store_path 选根与 Cloud 端到端测试，本项外置）。

## 现状
- 每个 shard 在 `fkv_shard::StoreBackend::cloud` 内持一个
  `CloudStore<CompioStore, S3ObjectStore>`，`cache_budget` 都取
  `local_space_limit`（同一上限被 N 个 shard 各自执行）。
- 运行时侧另建实例：
  - `PrewarmCrond`：`open_runtime_cloud_store`（长期持有，会下载文件、记
    `resident_bytes`）；
  - `ArchiveCrond`：只调 `remote_partition_page` 列举，不写本地 cache；
  - `Runtime::enumerate_targets`：每次 `global_*` 新建一个，只列举。
- `CloudStore::resident_bytes` 是实例内 `CacheState` 计数，实例之间互相看不见：
  - 列举型实例不写本地文件，实际不占预算；
  - 真正的双重记账在 prewarm 实例与 shard 实例（同一本地根、同一批文件）；
  - 多 shard 时上界是 `(N_shard + 1) × local_space_limit`，不是文中早先写的
    固定 2×。

## 为什么不在 12c 内硬做
1. 真正的统一需要把 prewarm / 列举搬到 shard 内，即
   `Request::Prewarm` / `Request::ListPartitions`（`issue/todo/12a-per-table-request-gaps.md`）。
   `Request` 是穷尽 `match`（`fkv_shard::dispatch` / `op_of` / `is_read_op` /
   `is_write_op`），新增变体会同时改 `fkv_api` / `fkv_shard` / `fkv_metrics`
   三个并发热点；12a 当前仍被这些热点阻塞。
2. “共享原子预算”（所有实例共用一个 `Arc<AtomicU64>`）只能统一记账，不能
   严格约束磁盘：每个实例只能从**自己的 LRU** 淘汰，无法删除其他实例缓存的
   文件；总超限但本实例无 victim 时仍会突破预算。
3. 归档 crond / 全局列举实例只读远端，不应为它们再引入一份 cache 记账；
   给这类实例设 `cache_budget` 或共享计数器都是噪声。

## 方案（后续择一）
- 首选：按 `12a` 落地 `Request::Prewarm` / `Request::ListPartitions`，预热和
  列举由 shard 自己的 `CloudStore` 执行，运行时侧不再建实例。`prewarm_report`
  改为聚合 shard 回执。依赖 12a 的请求面批次（`fkv_api` + `fkv_shard` +
  `fkv_metrics`）。
- 备选：`fkv_shard` 暴露共享 `CloudStore` 的服务化句柄（通道收发
  `ListPartitions` / `Prewarm`），运行时侧通过句柄查询，不持有实例。
- 若只做预算：给 `CloudStore` 加 `cache_group`（共享 `resident_bytes` +
  按路径的跨实例淘汰表），并让 `backend_of` 把同一 `local_space_limit` 按
  实例数切分；这是近似约束，需明确文档化“非严格上界”。

## 验收
- Cloud 模式下 runtime 侧不再持有与 shard 平行的 `CloudStore`（prewarm /
  列举均经 shard）。
- `local_space_limit` 是整机本地 cache 上界：evict 后磁盘占用 ≤ 预算
  （测试可用小预算 + 多分区写/读驱动）。
- 归档 crond 与 `global_*` 仍能覆盖远端未 cache 分区（现有
  `fkv_runtime/tests/cloud_e2e.rs` 不回退）。

## 重叠
- `issue/todo/12a-per-table-request-gaps.md`（`Request::Prewarm` /
  `Request::ListPartitions`）。
- `issue/done/cld-47a-prewarm-background-trigger-and-cross-shard.md`
  （预热跨 shard 触发）。
- `issue/done/cld-51b-cache-evict-off-hot-path-background.md`（cache 淘汰移出
  热路径）。

## DONE（2026-11-05）

### 选择
采「共享同一个预算/记账对象」（不改 `fkv_api` 请求面，避开 12a / shard 热点）。

### 改动
- `fkv_cloud`：
  - 新增 `CacheGroup { limit, resident_bytes: AtomicU64 }`（`0` = 不限）；
    `CloudStore::with_cache_group(Arc<CacheGroup>)` / `cache_group()`；
    `with_cache_budget` 仍为本实例独占一组，单实例语义不变。
  - 所有 resident 变更点（data/segment 的 upsert、mark_clean、forget、
    forget_table_cache、take_victim）同步 `reserve/release/adjust` 整组记账；
    `cache_remained()` / `trim_cache()` / `evict_drain_due()` 的阈值改按整组
    用量，任一实例写入都向同一个 `local_space_limit` 收敛。
  - `CacheGroup::release` 用 CAS 饱和减法（`fetch_update` 已弃用、
    `try_update` 高于 clippy.toml 的 msrv=1.89）。
- `fkv_shard`：`CloudBackend.cache_budget` → `cache_group: Arc<CacheGroup>`；
  `StoreBackend::cloud` 直接共享该组。
- `fkv_runtime`：`backend_of` 用 `local_space_limit` 建**一个**
  `CacheGroup`；shard 克隆 `Backend` 与 `open_runtime_cloud_store`
  （prewarm crond / archive crond / `global_*` 列举）都复用同一 `Arc`。
  因此整机上界从 `(N_shard + 1) × local_space_limit` 收敛到
  `local_space_limit`。

### 测试
- `fkv_cloud/tests/cloud.rs::cache_group_bounds_parallel_instances_to_single_budget`
  （MemStore + MemObjectStore，无需 S3）：
  - 3072B 预算、两实例各写/读 2048B 整文件；共享组下整组驻留与实际落盘
    字节（两本地后端之和）均 ≤ 3072B；
  - prewarm 实例看到的是整组余量（1024B），不是自己那份 3072B；
  - 只跑 `remote_partition_page` 不改变整组记账；
  - 对照组两个 `with_cache_budget(3072)` 实例落盘 4096B，复现 2× 上界，
    证明测试有区分力。
- `fkv_shard/tests/cloud_cache_metrics_wiring.rs`：断言
  `Arc::ptr_eq(store.cache_group(), &cloud.cache_group)`。
- `fkv_runtime` 单测：`backend_of` 的组上限 = `local_space_limit`；
  `open_runtime_cloud_store` 与 `CloudBackend` 共享同一 `Arc`。
- 已验证：`cargo test -p fkv_cloud`、`fkv_shard --test
  cloud_cache_metrics_wiring`、`fkv_runtime --lib / --test cloud_e2e /
  --test prewarm`、`cargo clippy -p fkv_cloud -p fkv_shard -p fkv_runtime
  --all-targets`。

### 边界与剩余
- 预算只约束**可淘汰的 clean cache**：dirty（未 flush）文件不入 LRU，
  旧单实例语义也是超预算后等 flush → trim 收敛，不因共享而变差。
- 淘汰仍只能摘本实例 LRU：写入方没有 victim 时只把自己刚写入的 clean
  条目摘回（self-evict），整组不越界，但可能牺牲利用率。
- 待删队列在 `drain_evictions` 前仍占盘，最多多出 `limit / 8`
  （既有语义）；drain 后严格 ≤ 预算。
- 实例 drop 不主动归还记账（保守方向：只会早淘汰）；共享组与 runtime
  同生命周期。
- 未消除「运行时侧仍持有一个平行实例」：prewarm / 列举搬进 shard 仍需
  `12a` 的 `Request::Prewarm` / `Request::ListPartitions`（`fkv_api` /
  `fkv_shard` / `fkv_metrics` 热点）。本项先把预算上界从
  `(N_shard + 1) ×` 收敛到 `1 ×`。

## 复审（2026-11-05 r6b）

通过。实现与测试全部核实：

- `CacheGroup`：`crate/engine/fkv_cloud/src/lib.rs:392-397`（struct）、`:504 with_cache_group`、`:510 cache_group()`；`with_cache_budget(498)` 仍单实例独占一组。
- shard 共享：`crate/engine/fkv_shard/src/backend.rs:44`（`cache_group: Arc<CacheGroup>`）、`:119 with_cache_group(clone)`。
- runtime 单组：`crate/engine/fkv_runtime/src/lib.rs:904`（`CacheGroup::new(option.local_space_limit)` 唯一建组点）、`:811` `open_runtime_cloud_store` 复用同组；单测 `:2083`（组上限 = local_space_limit）、`:2128-2129`（`Arc::ptr_eq` 断言运行时侧与 shard 共享）。
- 测试：`crate/engine/fkv_cloud/tests/cloud.rs:250-313` `cache_group_bounds_parallel_instances_to_single_budget`，断言整组驻留 ≤3072（:286）、实际落盘两后端之和 2048（:288-289）、对照组复现 2× 上界 4096（:311-312），区分力充分。
