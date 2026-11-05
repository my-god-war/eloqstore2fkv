
## 复审（2026-11-05 r6）

通过。`fkv_btree/src/lib.rs:116` byte_len 计 `alloc_len(page.capacity()) + ENTRY_ALLOC_OVERHEAD`（:124-131 ALLOC_GRANULE=16/粒度取整）、`:386` evict_to_fit、`:416` trim_to_budget；测试 `:967 single_oversized_entry_is_not_cached`、`:977 referenced_entries_still_evicted_after_two_rounds`。
