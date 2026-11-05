# cld-55 P2：`CloudStore::with_metrics` 零生产接线，cloud cache 四个指标恒 0

来源：第 5 轮 runtime/cloud lane 侦查，协调者亲验。

## 证据（2026-11-05 核验）

- 定义：`crate/engine/fkv_cloud/src/lib.rs:377-398` `pub fn with_metrics`。
- 全仓调用点仅 `fkv_cloud/tests/cloud.rs:268`（测试）。
- 两个生产装配点都不传 metrics：
  `fkv_shard/src/backend.rs:83-104`（`StoreBackend::cloud`）、
  `fkv_runtime/src/lib.rs:660-687`（`open_runtime_cloud_store`）——
  只 `.with_segment_config(...)` / `.with_cache_budget(...)`。
- `ShardMetrics` 的 `inc_cache_hit/inc_cache_miss/inc_cache_evict/set_cache_bytes`
  （`fkv_metrics/src/lib.rs:306-335`）就绪但 cloud 内部 `metrics: None` 空转，
  `evict_to_budget` 的 `inc_cache_evict()` 同样黑掉。

## 问题

Cloud 模式本地 cache 命中率 / 淘汰 / 驻留字节完全不可观测；`with_metrics`
同时是孤儿导出（unused.js 语义上的零引用，工具因测试引用未报）。

## 目标

shard 装配点（`backend.rs`）把该 shard 的 `Arc<ShardMetrics>` 接进
`CloudStore`；运行时侧实例（prewarm/crond/全局列举）无 shard 归属，
保持 `None` 并在注释说明。接线后 `cache_bytes` 口径与
`doc/metrics.md` 对账（index cache 相加逻辑已有先例）。

## 验收

- Cloud 单测：预热后 `snapshot().cache_hit > 0`、预算触发后 `cache_evict > 0`。
- `cargo nextest run -p fkv_cloud -p fkv_shard`。

## 依赖 / 重叠

- sto-60 是指标命名/表面错位（已产出 doc/metrics.md），本条是接线缺失，互补。
- 改 `fkv_shard/src/backend.rs`（并行会话热点），开发前对齐 owner。

## 核验记录

- 本人实读：grep 全仓 `with_metrics`、`fkv_metrics/src/lib.rs:306-335`。
- 子代理摘录（未复核）：`backend.rs:83-104`、`fkv_runtime:660-687` 装配细节。

## 审查结论（2026-11-04，协调者复核）

**采纳**（已核对当前源码，断言成立，留在 todo 待开发）：

- `cld-55`：`rg with_metrics` 全仓命中只有 `fkv_cloud/src/lib.rs:377` 的定义与
  `fkv_cloud/tests/cloud.rs:268` 的测试；`fkv_shard/src/backend.rs` 装配
  `Backend::Cloud` 时不接 `ShardMetrics` → cloud cache 的 hit/miss/evict/bytes 恒 0。
  接线点在 `fkv_shard`（shard 线程内有 `Arc<ShardMetrics>`）+ `CloudBackend` 参数。
- `io-39`：`KvError::EndOfFile` 只有 `fkv_error/src/lib.rs` 的定义与文案，
  `fkv_io` / `fkv_io_compio` 里零使用；读路径把 EOF/短读耗尽归成 `Corrupted`
  （证据行复核见正文）。

## DONE（2026-11-04，dragon 落地）

**接线完成（fkv_shard 属本 lane，fkv_cloud 改动为纯追加）。**

- `fkv_shard/src/backend.rs`：`StoreBackend::cloud(config, cloud, metrics)`
  末接 `.with_metrics(metrics)`，并在文档注释写明「不传则四项指标恒 0」。
- `fkv_shard/src/lib.rs`：`build_backend(config, metrics)` 增加 `&Arc<ShardMetrics>`
  参数（调用点传 `thread_metrics`），Cloud 分支透传；Local / Mem 不受影响。
- `fkv_cloud/src/lib.rs`：新增 `pub fn metrics(&self) -> Option<&Arc<ShardMetrics>>`
  （文档注明只给装配点自检用，避免为测试加行为 API）。
- `fkv_runtime`：`open_runtime_cloud_store` 的文档补一段说明
  **有意不接** `ShardMetrics`（运行时侧实例不归属任何 shard，挂某个 shard 的
  计数器会让命中率指向错误的 shard），实现保持 `None`。

**验证**：

- 新增 `fkv_shard/tests/cloud_cache_metrics_wiring.rs`：用不可达 endpoint 只建
  S3 客户端（不发请求），断言 `StoreBackend::Cloud` 内的 `metrics()` 与本 shard 的
  `Arc<ShardMetrics>` **`Arc::ptr_eq`**。刻意用指针相等而不是计数：**「没接线」与
  「cache 一直没命中」在计数上无法区分**。
  **反向验证**：去掉 `.with_metrics(metrics)` → 测试 FAIL（报「未接线」）；加回 PASS。
- 计数行为侧已有 `fkv_cloud/tests/cloud.rs::cache_metrics_record_hit_miss_and_eviction`
  （MemObjectStore，断言 `cache_evict >= 1`、`cache_miss >= 1`、`cache_hit >= 1`、
  `cache_bytes == resident_bytes()`）——即本条验收里的「预热后命中 / 预算触发淘汰」，
  现在由 shard 装配点把实例真正接上，不再恒 0。
- `cargo nextest run -p fkv_shard -p fkv_cloud` **69 passed**；
  `cargo clippy -p fkv_shard -p fkv_cloud -p fkv_runtime --all-targets --no-deps -- -D warnings`
  EXIT=0；全量 `./test.sh` **573 passed / 1 skipped**。

**未做（记档）**：cloud cache 的 `set_cache_bytes` 目前报的是「本实例驻留字节」，
与 `doc/metrics.md` 里 index cache 的口径（各表相加）不是同一分母；等
`sto-60` 的 gauge 族收口时一起对账（那是另一条已挂 todo 的线）。

## 复审（2026-11-05 r6）

**通过。** 核实：

- `fkv_shard/src/backend.rs:102-118` `StoreBackend::cloud(config, cloud, metrics)` 末尾 `.with_metrics(metrics)`（:118）；`fkv_shard/src/lib.rs:221` `build_backend(config, metrics)` 贯通。
- `fkv_cloud` 侧 `metrics()` 自检导出在位；运行时侧 `open_runtime_cloud_store` 有意不接（:767 注释区），记档一致。
- 测试 `fkv_shard/tests/cloud_cache_metrics_wiring.rs:58` `cloud_backend_attaches_shard_metrics`：:88 `Arc::ptr_eq(attached, &metrics)` —— 指针相等断言确实区分「未接线」与「未命中」；计数行为侧 `fkv_cloud/tests/cloud.rs:256` `cache_metrics_record_hit_miss_and_eviction`。
