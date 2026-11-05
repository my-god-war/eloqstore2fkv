
## 复审（2026-11-05 r6）

通过（本条落点在 fkv_task_write/fkv_btree，非本 lane crate，抽验关键符号）。

- `fkv_task_write/src/lib.rs:214-230`：去重改为 `Vec<&BatchEntry>` 稳定
  `sort_by` 按 key 升序 + 写指针原地折叠，同 key 取 timestamp 较新者
  （相等取后者），全程零 entry/value clone，与方案一致。
- 测试 `tests/write.rs:134 duplicate_keys_keep_newest`。
- `merge_data_pages*` 泛型化（BatchView 接受引用切片）由 eng-62a 侧承接，
  与 md 交叉引用一致。
