# eng-66 本地模式 IndexCache 是指标黑洞：cache_hit/miss/evict/bytes 只接了 fkv_cloud，容量旋钮 with_capacity_bytes 无生产调用

## 位置
- Rust: `crate/foundation/fkv_metrics/src/lib.rs:379-382`（`ShardSnapshot` 定义了 `cache_hit/cache_miss/cache_evict/cache_bytes`）；全仓唯一调用点在 cloud block cache（`crate/engine/fkv_cloud/src/lib.rs:116-130`）；`crate/engine/fkv_shard/src/lib.rs:255-257`（`metrics_snapshot()` 不读 `index_cache_map`）；`crate/core/fkv_btree/src/lib.rs:126`（`DEFAULT_INDEX_CACHE_BYTES = 64 MiB` 硬编码为 Default）；`crate/engine/fkv_store/src/lib.rs:271,871,1263`（全部 `IndexCache::default()`，`with_capacity_bytes` 除测试外零调用）
- C++: `include/storage/page_manager.h:119-139`（`DataCacheHits/Misses/IncrDataCacheHit/Miss`，单 shard 单写者 relaxed 原子）；`src/storage/page_manager.cpp:152-160`（`GetBufferPoolUsed/GetBufferPoolLimit`）；`src/eloq_store.cpp:2735-2755`（跨 shard 聚合导出）；`src/storage/shard.cpp:1626-1630`（workloop 周期上报 `INDEX_BUFFER_POOL_USED`）

## 现状
本地模式命中率、逐出数、缓存占用恒为 0；每表 64 MiB × N 表聚合占用无上限也无观测点。

## C++ 实现
命中/未中/占用是常导出指标，池上限可配（`buffer_pool_size`）。

## 差异与风险
性能验收（命中率、RSS 对比）无从做起；缓存行为不可观测。

## 对齐建议
- `IndexCache` 内建 hit/miss/evict 计数（`Cell<u64>` 即可，单线程访问）；
- `Shard::metrics_snapshot` 汇入 `cache_*` 字段（cloud 与 index 两来源相加或分列）；
- `ShardConfig`/`KvOption` 暴露 index cache 字节预算接 `with_capacity_bytes`。

## 严重度
P2（可观测性与配置面）

## 核验记录（2026-11-04）
metrics 字段、cloud 唯一调用点、IndexCache::default() 三处锚点实读/全仓 grep 确认；C++ 侧引用自子代理摘录。

## 审查结论（2026-11-04）

- 锚点核验通过：`ShardSnapshot` 字段、cloud 唯一写入点、`IndexCache::default()` 三处、
  `with_capacity_bytes` 无生产调用，均与 issue 描述一致。
- 判定：合理（P2 可观测性与配置面），已拆分实现并归档：
  - `eng-66a-index-cache-metrics-wiring.md`：IndexCache 自身计数 + shard 指标同步。
  - `eng-66b-index-cache-bytes-knob.md`：KvOption → ShardConfig → Store 预算链。
- 与 eng-69 的边界：统一 index+data 池模型与 `index_buffer_pool_limit` gauge 仍属 eng-69，
  本次只上报 index cache 的 `used` 与独立预算。
- 与 eng-67 的边界：reopen 是否保留缓存内容属 eng-67；本次计数器用 store 级共享
  `Arc<CacheCounter>`，保证 reopen 后累计值不回退。
- eng-70 已将 `IndexNode` 改为按需 materialize，`byte_len` 仍按结构 + 常驻页统计，
  本次沿用，未改预算语义。

## 复审（2026-11-05 r6）

通过（父条目）。两项拆分均已落地并按当前源码复核，详见 `eng-66a` / `eng-66b` 的 r6 复审。
