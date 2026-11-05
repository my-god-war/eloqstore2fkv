
## DONE（2026-11-04）
- 实现：抽出 `Store::rebalance_cache_budget`，`ensure_index_cache` 与 `install_replay_state` 共用；replay 安装末尾（mapper / 代际登记之后）立即裁剪全表缓存，不再等下一次写。
- 测试：`crate/engine/fkv_store/tests/index_cache.rs::reopen_rebalances_cache_budget`（A 独占整池 → evict 后 reopen B → 只读，断言 A 已收缩、聚合 ≤ 总预算；B 填满后仍不越界）。已确认去掉修复后该用例失败。
- 验证：`cargo check -p fkv_store -p fkv_btree -p fkv_mapping` 通过；`cargo nextest run -p fkv_store -p fkv_btree -p fkv_mapping -p fkv_runtime --no-fail-fast` 201 passed；三 crate clippy `--no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:364` `rebalance_cache_budget`，`:357`（ensure_index_cache）与 `:1851`（install_replay_state 末尾）两处调用；测试 `tests/index_cache.rs:327 reopen_rebalances_cache_budget`（TOTAL=4096 双表断言）。IndexCache 本体在 fkv_btree（本 lane crate），实现与断言均锁定预算不越界。
