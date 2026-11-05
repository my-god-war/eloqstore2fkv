# io-36 `write_page_li` 的连续组合不按 `max_write_batch_pages` 切分：单组页数无上限，配置项全仓零消费

## 位置

- `crate/io/fkv_io_compio/src/lib.rs:1287-1369`（`write_page_li`）：
  - `:1310-1338`：排序后把同一文件内**所有连续页**合成一个 group，
    没有页数/字节上限；`bytes = count * page_size`。
  - `:1361-1363`：`buffer_unordered(WRITE_INFLIGHT_LIMIT)` 限制的是“组数”（8），
    不是“页数”；一组 128 页与一组 1 页占同一个并发名额。
- `crate/io/fkv_io_compio/src/lib.rs:54`：`WRITE_INFLIGHT_LIMIT = 8` 是独立常量，
  与 `KvOption::max_write_batch_pages` 无关。
- `crate/foundation/fkv_config/src/lib.rs:30,101,271`：`max_write_batch_pages`
  默认 32、可从 INI 解析；全仓无消费者（`ShardConfig` / `CompioStore` 都没有该字段）。
- 调用侧一次可能传很多页：`crate/engine/fkv_task_write/src/lib.rs:699`（数据页分裂）、
  `:864`（overflow 链）、`:1116`（index 页分裂）、`:1141`（逐层重建 index）；
  `write_new_data_page` 一批 batch 可生成多页 data。

## 现状

- “连续页合并成一次 `write_at`”本身正确；问题在于合并组没有上限，单组缓冲大小
  随连续页数线性增长。大 batch / 全树重建 / 单文件内大量新页时，一次 flush
  可分配数 MB 到数百 MB 的连续缓冲。
- C++ 的 `max_write_batch_pages` 是硬性 in-flight 页数上限；Rust 解析了配置却从不使用，
  实际行为与 `KvOption` 承诺不一致。
- 缓冲池、注册写槽位、组间并发属于 io-27 / io-03 的范围；本条只补“页数上限 +
  配置接线”，避免重复。

## C++ 实现

- `include/kv_options.h:83`：`max_write_batch_pages = 32`。
- `src/tasks/write_task.cpp:305`：`if (inflight_io_ >= opts->max_write_batch_pages)` 才等待，
  即同一时刻最多 32 个页写 SQE；连续页合并也在该上限内切批。
- `src/async_io_manager.cpp:178-183`：`WriteReqPool` / `MergedWriteReqPool` 以
  `max_inflight_write` 为容量，写请求对象池天然限制未完成请求数。

## 建议

1. 给 `CompioStore` 增加 `max_write_batch_pages: usize`（默认 32），由
   `ShardConfig` / `KvOption` 传入；`Shard::start` 与 `KvStore::new` 都接线。
2. 在 `write_page_li` 的连续 run 扫描里加页数上限：
   ```rust
   let max_pages = self.max_write_batch_pages.max(1);
   while end - start < max_pages && sorted[end].0 == file_id
       && sorted[end].1 == sorted[end - 1].1 + 1 { end += 1; }
   ```
   `WRITE_INFLIGHT_LIMIT` 继续限制组并发；总 in-flight 页数由 `max_pages` 约束。
3. 把 `:1288-1294` 的逐页 failpoint 提到批外，触发一次 `WritePageBatch`（可选，
   避免页数多时注入点数量与 C++ 不一致）。
4. 回归测试：100 个连续页的 `write_page_li`，断言 `write_at_num` 按 `max_pages`
   切分、单组 `bytes <= max_pages * page_size`；再改小 `max_write_batch_pages=4`
   断言切分数正确。
5. 如果 io-27 的注册写槽位先落地，这里的切批上限仍然需要：槽位池是“缓冲来源”，
   `max_write_batch_pages` 是“提交页数上限”，两者独立。

## 严重度

P2（单组缓冲无界 + 配置项未接线；与 io-27 的缓冲池/并发问题不重复）

## 核验记录（2026-11-04）

`write_page_li` 当前 `:1287-1369` 的分组循环与 `buffer_unordered(8)` 实读；
`max_write_batch_pages` 在 config 的三处定义与全仓零消费 grep 确认。
C++ `write_task.cpp:305` 的 in-flight 页数上限与 `kv_options.h:83` 默认值实读。

## 审查结论（2026-11-04）

核验结果：**合理**，行号已漂移但结论成立。

- `write_page_li` 现位于 `crate/io/fkv_io_compio/src/lib.rs:1479-1553`；连续 run 扫描
  （`while end < sorted.len() && sorted[end].0 == file_id && ...`）确实无页数上限，
  `bytes = count * page_size` 随连续页数线性增长。
- `WRITE_INFLIGHT_LIMIT = 8`（`:51`）只约束组并发，一组的页数不受限。
- `max_write_batch_pages` 仅在 `crate/foundation/fkv_config/src/lib.rs:30,101,194,271`
  出现，`ShardConfig` / `CompioStore` 均无该字段，全仓零消费属实。
- 与 io-27（缓冲池 / 并发）不重复：本条只补“页数上限 + 配置接线”。
- 建议 3（failpoint 提到批外）为可选，逐页注入点与 C++ `WriteTask` 每页调用语义一致，
  本次不改，避免改变现有 failpoint 计数测试（`tests/batch.rs:68`）。

## TODO（2026-11-04）

1. `CompioStore` 增加 `max_write_batch_pages: usize`（默认 32 = C++
   `kv_options.h` 值），提供 `with_max_write_batch_pages`；
   `write_page_li` 的连续 run 扫描按该值切分，单组 `bytes <= max_pages * page_size`，
   `WRITE_INFLIGHT_LIMIT` 继续限制组并发。
2. 配置接线（沿用既有链路）：`KvOption::max_write_batch_pages`
   -> `ShardConfig.max_write_batch_pages`（`Shard::start` 转 `with_max_write_batch_pages`）
   -> `CompioStore`；`fkv_sdk::KvStore::new` 同步接线。
3. 回归测试（`crate/io/fkv_io_compio/tests/batch.rs`）：
   100 连续页默认 32 页/组 => 4 次 `write_at`；`with_max_write_batch_pages(4)` => 25 次；
   末组余数正确，读回内容一致。

## DONE（2026-11-04）

实现要点：

- `crate/io/fkv_io_compio/src/lib.rs`：
  - 新增 `DEFAULT_MAX_WRITE_BATCH_PAGES = 32` 常量与 `CompioStore::max_write_batch_pages` 字段；
  - 新增 `with_max_write_batch_pages(pages)` 构造器（0 归一为 1）；
  - `write_page_li` 连续 run 扫描加 `end - start < max_pages` 上限，单组
    `bytes = count * page_size <= max_pages * page_size`；`WRITE_INFLIGHT_LIMIT`
    仍只管组并发。
- 配置接线（沿用既有链路，无平行配置）：
  - `fkv_config`：新增 `DEFAULT_MAX_WRITE_BATCH_PAGES`，`KvOption::default` 复用；
  - `fkv_runtime`：`KvOption::max_write_batch_pages` -> `ShardConfig.max_write_batch_pages`；
  - `fkv_shard`：`ShardConfig.max_write_batch_pages` -> `CompioStore::with_max_write_batch_pages`；
  - `fkv_sdk::KvStore::new`：同链路直接接线。
- 未改 failpoint 位置（建议 3 标注为可选；逐页注入与现测试一致）。

测试：
- `crate/io/fkv_io_compio/tests/batch.rs::contiguous_pages_split_by_max_write_batch_pages`
  100 连续页：默认 32 -> 4 次 `write_at`；上限 4 -> 25 次；上限 7 -> 15 次（余数）；
  逐页读回内容一致。

验证：
- `cargo nextest run -p fkv_io_compio`：30 passed
- `cargo nextest run -p fkv_shard -p fkv_config`：8 passed
- `cargo nextest run -p fkv_runtime -E 'test(options)'`：1 passed
- `cargo clippy -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings`：0 warning

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:69 DEFAULT_MAX_WRITE_BATCH_PAGES=32`、字段
  `max_write_batch_pages`（`:629`）、`with_max_write_batch_pages`（0 归一 1，
  `:854-857`）；`write_page_li` 连续 run 扫描带 `end - start < max_pages` 上限
  （`:2064-2073`），单组 `bytes = count * page_size`（`:2078`）。
- 接线链：`fkv_config/src/lib.rs:43 DEFAULT_MAX_WRITE_BATCH_PAGES`、
  `:186 max_write_batch_pages`；`fkv_runtime/src/lib.rs:114 write_limits` →
  `ShardConfig`（`:743/756/790/1099/1136`）。
- 测试 `tests/batch.rs:98 contiguous_pages_split_by_max_write_batch_pages`
  （100 页默认 4 次 / 上限 4 → 25 次 / 上限 7 → 15 次余数，逐页读回一致）。
- `WRITE_INFLIGHT_LIMIT` 已由 io-40 演进为与 `max_inflight_write` 合一
  （`:2099-2103`），属后续修正。
