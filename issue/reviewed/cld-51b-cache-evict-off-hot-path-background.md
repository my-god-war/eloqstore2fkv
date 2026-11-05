# cld-51b 把 cache 淘汰删除移出读写热路径（后台 FileCleaner）

## 位置
- `crate/engine/fkv_cloud/src/lib.rs`：`evict_to_budget`（`local.remove_file` /
  `local.remove_segment_file` 同步 `await`）、热路径调用点
  `read_page` / `read_page` 命中 / `read_page_li` / `read_segment` /
  `write_page` / `write_segment` / `flush_tbl` / `flush_segment_tbl`。
- C++：`async_io_manager.cpp:7089-7122`（后台 `FileCleaner` 批量 unlink）。

## 现状
- cld-51a 之后 victim 选取已 O(1)，但 `evict_to_budget` 仍在读写路径上同步
  `await` 本地文件删除。

## 阻塞原因（未实现）
- `CloudStore` 是 `RefCell` 单线程值语义、`new()` 返回 `Self`，无 `Arc` /
  执行器句柄；`fkv_cloud` 不依赖任何 runtime（仅 `futures-timer`），
  无法安全 `spawn` 一个 `'static` 删除任务。
- 依赖 `12-runtime-modes-api`（attach 时注入后台执行器）与 `cld-47`
  后台维护任务落地后可复用同一执行器；届时：
  1. `evict_to_budget` 只把 victim 从 LRU 摘链并推入待删队列（同步 O(1)）；
  2. 唤醒后台 FileCleaner 批量 `remove_file` / `remove_segment_file`；
  3. `resident_bytes` 在摘链时立即扣减，语义与当前一致。
- 在落地前保持同步删除，避免“已扣字节但文件还在”导致重复下载的中间态。

## 验收（前置依赖解除后）
- 读写路径不再 `await` 本地删除；`evict_to_budget` 返回时仅完成摘链。
- 现有预算语义测试全绿（删除最终生效，允许后台延迟）。

## 阻塞复核（2026-11-04 10:05）

复核命令与结果：

- `fkv_cloud` 无运行时：`crate/engine/fkv_cloud/Cargo.toml` 依赖仅
  fkv_error / fkv_io / fkv_metrics / fkv_object / fkv_types / futures-timer / gxhash；
  `grep -n "spawn\|Runtime\|Handle\|executor" crate/engine/fkv_cloud/src/lib.rs`
  → **0 命中**。`CloudStore::new() -> Self` 值语义，拿不到 `'static` 执行器句柄，
  无法自行 spawn FileCleaner。
- 落点在 lane 外：注入后台执行器/cleaner 的调用方是 `fkv_shard`（attach /
  维护循环）与 `fkv_runtime`，均不在本 lane 允许的 crate 列表内。
- 不能只做一半：把 `evict_to_budget` 改成「只摘链 + 推待删队列」而无人 drain，
  读多写少（无 flush）时本地磁盘会长期超出 `cache_budget`——预算是**磁盘**约束，
  这会把「淘汰保证」换成「字节记账保证」，属行为回退，故不做。
- 也不加「注入 seam + 默认内联」的预留接口：默认路径虽在使用，
  但 setter 零调用方即 `pub API 孤儿`，违反 rust_review 的零死代码条款。
- cld-51a（已 done）已把 victim 选取降到 O(1)（`evict_candidate` 读 `lru_head`，
  `crate/engine/fkv_cloud/src/lib.rs:884-896`），本 issue 只剩「删除离开热路径」
  这一半，且它依赖 `12-runtime-modes-api`（仍在 `issue/new/`，未认领）。

## DONE（2026-11-05）

不做后台线程：`CloudStore` 仍无 runtime / `'static` 句柄，改为**同步摘链 +
阈值批量删除 + 显式 drain**，不依赖 `12-runtime-modes-api` / `cld-47`。

实现要点（`crate/engine/fkv_cloud/src/lib.rs`）：

1. `trim_cache()`：超预算时只从全局 LRU 头摘链、删 `table_map` 条目、
   立即扣 `resident_bytes`，victim 推入 `CacheState::pending_delete_li`
   （`pending_delete_bytes` / `evict_seq` 记账），全程无 IO。
2. `take_victim()`：O(1) 取下 head 并摘链，复用 cld-51a 的侵入式 LRU；
   dirty entry 本就不在链上，淘汰仍只作用于干净文件。
3. `pub async fn drain_evictions()`：批量执行 `remove_file` /
   `remove_segment_file`。删除前复核 `is_resident`：淘汰后同文件若已被重新
   下载 / 写入，取消删除，避免误删刚回填的 cache。失败项重新入队并恢复
   `pending_delete_bytes`，显式 drain 返回首个错误，记账不丢。
4. `drain_evictions_if_due()`：热路径只在待删队列达到
   `cache_budget / 8`（`EVICT_DRAIN_SHIFT = 3`）或 1024 个文件时批量删除；
   删除失败只 `warn` 并把 `drain_seq` 追上，坏盘时不会每个 IO 重试。
5. 热路径 `read_page` / `read_page_li` / `read_segment` /
   `write_page` / `write_page_li` / `write_segment` / fixed 变体 /
   `read_file` / `write_file` / prewarm 全部改为
   `trim_cache()` + `drain_evictions_if_due()`：单次 IO 不再逐 victim
   `await` 本地删除；`flush_tbl` / `flush_segment_tbl` 在上传完成后全量
   `trim_cache()` + `drain_evictions().await`，作为自然批量删除点。
6. 预算上界：摘链时同步扣 `resident_bytes`，返回前仍满足
   `resident_bytes <= cache_budget`（dirty 文件仍在时与旧语义一致，只受
   dirty 保护限制）；本地磁盘正常路径最多多出待删队列的约 `budget / 8`
   （或 1024 个小文件）字节，达到阈值后由触发 IO 批量删除，不会无界增长。

测试（`crate/engine/fkv_cloud/tests/cloud.rs`）：

- `cache_evict_batches_deletes_and_keeps_refilled_file`：待删 512B < 阈值
  时本地文件延迟删除；累计到阈值后批量删除；淘汰后重新写入的 file 0
  被 drain 跳过、file 1 正常删除。
- `drain_evictions_flushes_pending_deletes`：未达阈值时显式 drain 立即删除。
- `drain_evictions_requeues_failed_delete`：`FlakyRemoveStore` 注入 unlink
  失败，热路径不报错、victim 回队；显式 drain 重试成功。
- 既有预算 / LRU / metrics 测试不变：阈值小于单文件大小时自动 drain，
  旧的「sync 后本地文件已删」断言继续成立。

验证：

- `cargo test -p fkv_cloud` 全绿（cloud 25 + segment_manifest 6 + 其它目标）。
- `cargo clippy -p fkv_cloud --all-targets` 零警告。
- `cargo +nightly fmt -p fkv_cloud`。

剩余风险：

- 无后台任务时，删除仍由业务 IO 在阈值点批量触发；若进程在 pending
  窗口内崩溃，残留本地文件由下次 attach 的本地目录清理 / 后续淘汰兜底，
  与旧实现「删除后崩溃」的残留窗口同量级。
- 本地 unlink 持续失败时队列只保留在内存并等待显式 drain，热路径不再把
  清理失败升级为数据面错误；此时磁盘上界无法保证（底层 IO 已失效），
  但预算记账与失败项不丢。

## 复审（2026-11-05 r6）

**通过（按“同步摘链 + 阈值批量 + 显式 drain”折中方案）。** 核实：

- `fkv_cloud/src/lib.rs:75-77` `EVICT_DRAIN_SHIFT=3`（预算 1/8 阈值）；:1045 `trim_cache`（只摘链 + 推 `pending_delete_li`，立即扣 `resident_bytes`，无 IO）；:1106 `take_victim`；:1170 `pub async fn drain_evictions`（删除前复核 `is_resident` 防误删回填，失败重新入队）；:1227 `drain_evictions_if_due`（阈值批量，失败 warn + `drain_seq` 追上）。
- 测试 `tests/cloud.rs:936` `cache_evict_batches_deletes_and_keeps_refilled_file`、:960 `drain_evictions_flushes_pending_deletes`、:1017 `drain_evictions_requeues_failed_delete`，三条齐备且断言覆盖延迟删除/阈值触发/回填保护/失败重入队。
- 「不做后台线程」的理由（CloudStore 无 runtime 句柄）与剩余风险（pending 窗口崩溃残留）已如实记档。
