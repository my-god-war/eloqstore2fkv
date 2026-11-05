
## 复审（2026-11-05 r6）

通过。三项删除均经全仓 grep 复核：

- `seek_index`：`crate/core/fkv_btree` 全目录零命中；`tests/seek.rs:38` 改用
  `IndexNode::new(0, &page).seek(..)`。
- sleeper 族：`fkv_task_reopen/src/lib.rs` 现仅 133 行，只剩生产在用的
  `AutoReopenOption`(:95-108) 与 `reopen*`/`replay_manifest_bytes*`，无
  `Sleeper`/`ThreadSleeper`/`auto_reopen_with*`。
- `live_page_li`：`fkv_task_gc/src/lib.rs` 只有 `live_page_li_by_file`(:295)；
  `tests/gc.rs`、`fkv_store/tests/maintenance_batch.rs` 相应改用。
