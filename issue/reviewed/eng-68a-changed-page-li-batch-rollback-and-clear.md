
## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:560`（`BatchJournal.changed_len`）、`:699`（`begin_batch` 记录
`changed_page_li.len()`）、`:723`（`abort_batch` 按 journal 截断）、`:761-775`
（`take_changed_page_li` / 只读 `changed_page_li()` / `clear_changed_page_li` 保留容量）。
测试 `fkv_mapping/tests/mapping.rs:399`（`abort_batch_truncates_changed_page_li`）存在。
