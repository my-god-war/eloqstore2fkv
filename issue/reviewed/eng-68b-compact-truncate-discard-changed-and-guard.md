
## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:2354`（`compact_file` 落 log 成功后 `clear_changed_page_li`，
:2347 注释直接引用 eng-68）、`:1920/:1924`（`truncate` 两 mapper 在 `clear_dirty_page_li`
旁一并清除）、`:646-655`（`invalidate_index_cache` 消费后 `debug_assert!` 两个 mapper 的
changed 列表为空，以内联断言形式实现「assert_changed_consumed」语义）。测试
`fkv_store/tests/index_cache.rs:69`（`compact_keeps_index_cache_for_next_write`）与
`:121`（`truncate_discards_changed_page_li`）存在。
