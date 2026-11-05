
## 复审（2026-11-05 r6）

通过（落点 fkv_task_write，非本 lane crate，抽验关键符号）。

- `fkv_task_write/src/lib.rs:73 PENDING_WRITE_BUDGET = 1 << 20`、
  `:81 PendingWrite`、`:119 bytes >= 预算即 flush`、`:143/:240` 挂载于写路径。
- 测试 `tests/write.rs:1190 cross_group_writes_flush_once`（跨 5 data 页一批
  断言 write_page_li 调用 1 次）。
