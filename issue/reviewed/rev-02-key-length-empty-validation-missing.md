
## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:1295-1304`（`check_batch_keys`：`max_len = self.page_size >> 1`，
空 key 或超长整批 `InvalidArgs`）、`:1325`（`put_at` 首行调用，先于 `touch_table` 与
mapper 批次）。测试 `fkv_store/tests/store.rs:48`（`key_len_bounds_reject_whole_batch`）
存在。
