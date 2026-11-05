# io-29d fkv_task_prewarm 逐页列举 → 下载流水线

## 位置
- `crate/engine/fkv_task_prewarm/src/lib.rs`：`prewarm_table`。
- 测试：`crate/engine/fkv_task_prewarm/tests/prewarm.rs`。

## 现状（issue 遗留项）
`prewarm_table` 先 `remote_data_file_id_li` / `remote_segment_file_id_li`
收集全量 id，再开始下载：首对象可用时间 O(对象数)，内存 O(bucket)。
对齐目标：C++ `kMaxPrewarmPendingFiles` 的有界队列，边列举边下载。

## 方案
1. `file_id_stream(cloud, tbl, segment)`：`stream::unfold` + `VecDeque`，
   每页 `CLOUD_LIST_PAGE_KEYS` 条（1000）拉取，下游按需取用；
   在途下载数沿用 `PREWARM_FILE_CONCURRENCY`（8），
   因此 pending 上限 ≈ 一页 + 在途，O(页) 而非 O(bucket)。
2. `prewarm_table` 的数据 / segment 两阶段都改为消费该流；
   data 阶段非 Success 时不再进入 segment 阶段（沿用现语义）。
3. 列举失败不再整表 `Err`：计入 `files_failed` 并置
   `CompletionReason::ListingError`（该变体此前无生产者），
   已有下载计数保留，便于运维定位。
4. `total_files_listed` 在消费流时累加。

## 验收
- `cargo nextest run -p fkv_task_prewarm` 通过；新增 >1 页（1001 个
  data 对象）用例，断言全部列举并拉取。
- `cargo clippy -p fkv_task_prewarm --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

- `crate/engine/fkv_task_prewarm/src/lib.rs`：
  - `file_id_stream`：`stream::unfold` + `VecDeque`，每页
    `CLOUD_LIST_PAGE_KEYS` 条，按需拉取；内存 O(页)。
  - `prewarm_paged`：`buffer_unordered(PREWARM_FILE_CONCURRENCY)`，
    列举与下载并行，pending ≈ 一页 + 在途。
  - `prewarm_table` 数据 / segment 两阶段改为消费该流；
    列举失败置 `CompletionReason::ListingError` 并保留已完成的下载计数
    （此前该变体无生产者）；`total_files_listed` 在消费时累加。
- 测试：`crate/engine/fkv_task_prewarm/tests/prewarm.rs::prewarm_streams_across_list_pages`
  造 1001 个 data 对象（跨页），断言全部列举并拉取、无失败。
- 验证：`cargo nextest run -p fkv_task_prewarm` 3 passed；
  下游 `cargo nextest run -p fkv_task_reopen` 9 passed；
  `cargo clippy -p fkv_task_prewarm --all-features --all-targets --no-deps -- -D warnings` 零警告。
- 备注：全仓 `-D warnings` 仍会命中无关 crate（`fkv_mapping` useless_conversion、
  `fkv_task_read` too_many_arguments）的既有 lint，未在本 issue 范围内处理。

## 复审（2026-11-05 r6）

通过。

- `fkv_task_prewarm/src/lib.rs:58-96`：`stream::unfold` + `VecDeque`，每页
  `CLOUD_LIST_PAGE_KEYS` 条，按需拉取，内存 O(页)；
  `prewarm_paged`（`:112`）`buffer_unordered(PREWARM_FILE_CONCURRENCY=8)`
  （`:140`）；列举失败置 `CompletionReason::ListingError` 并保留已完成计数
  （`:155`）。
- 测试 `tests/prewarm.rs:118 prewarm_streams_across_list_pages`：1001 个 data
  对象跨页，断言全部列举并拉取、无失败。
