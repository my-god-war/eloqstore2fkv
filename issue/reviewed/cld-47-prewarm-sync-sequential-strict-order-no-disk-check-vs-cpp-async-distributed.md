
## 复审（2026-11-05 r6）

通过。已落地部分逐项核验：

- 并发：`fkv_task_prewarm/src/lib.rs:14/:140`（`PREWARM_FILE_CONCURRENCY=8` +
  `buffer_unordered`）、`:208-227`（`prewarm_tables_with_concurrency`，0 按 1 串行）。
- 流式列举：`:61-101`（`file_entry_stream`，`stream::unfold` 每页
  `CLOUD_LIST_PAGE_KEYS`，内存 O(页)）；`fkv_cloud/src/lib.rs:2110`
  （`remote_file_entry_page` 带 `RemoteFileEntry{file_id,size}`，:340）。
- 磁盘余量：`fkv_task_prewarm/src/lib.rs:129-131`（下载前
  `cloud.cache_remained() < entry.size` → `WarmErr::DiskFull` 中止）；
  `fkv_cloud/src/lib.rs:540`（`cache_remained`）。
- 测试：`fkv_task_prewarm/tests/prewarm.rs:144`
  （`prewarm_stops_when_cache_budget_exhausted`：预算 512B 装 1 个 512B 对象后
  DiskFull、resident_bytes 不超预算）、`:171`
  （`prewarm_tables_with_zero_concurrency_still_pulls_all`）。
- 不采纳项（mod_time 全局排序）与拆出项（cld-47a 后台触发）记录在案；cld-47a 另行复核。
