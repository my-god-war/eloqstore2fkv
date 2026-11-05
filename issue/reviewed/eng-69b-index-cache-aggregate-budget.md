
## 复审（2026-11-05 r6）

通过。`fkv_btree/src/lib.rs:141` CacheLedger、`:280` with_shared_budget、`:416` trim_to_budget、测试 `:995 shared_ledger_splits_total_between_caches`；`fkv_store/src/lib.rs:309` with_index_cache_total_bytes、`tests/index_cache.rs:186 aggregate_budget_shares_across_tables`（总量 4KiB 双表裁剪断言）。reopen 即时裁剪由 core-27 锁定。
