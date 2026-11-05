
## 复审（2026-11-05 r6）

通过（改判类条目：主张过时、仅补回归测试）。`fkv_store/tests/gc_store.rs:236`
（`maintain_clears_amplification_trigger`）存在，断言维护后 `needs_maintain` 为 false 且
`compact_if_needed` 返回空；`needs_maintain` 为 O(1) 派生预判（`mapping_count` 计数器 +
`space_size` 字段）与判定一致。
