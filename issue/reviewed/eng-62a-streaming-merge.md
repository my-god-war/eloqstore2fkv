# eng-62a merge 改双指针流式合并（去掉 BTreeMap 物化与 key/value 全量 clone）

## 位置
- `crate/core/fkv_btree/src/lib.rs`：`merge_data_pages_with_options` 及新增的
  `MergeVal` / `OldPageCursor` / `BatchView` / `PageSink` / `emit_batch`。
- `crate/core/fkv_btree/tests/btree.rs`：新增 3 条回归测试。

## 方案
1. 删除 `MergedEntry` + `BTreeMap<Vec<u8>, MergedEntry>` 物化路径。
2. 旧页用 `OldPageCursor` 跨页流式推进（持有 `DataPageIter`，不落容器）。
3. batch 用 `BatchView`：已排序直接借用；乱序只建 `Vec<usize>` 下标并稳定排序，
   不拷贝 key/value，重复 key 的 timestamp 折叠语义与旧实现一致。
4. `MergeVal` 借用旧页切片 / batch value；仅字典压缩时 `prepare` 产生一次分配。
5. `PageSink` 承接原 `DataPageBuilder` 切页 + overflow 占位逻辑，逐条直喂，
   无中间容器、无 `BatchEntry::clone`。

## 验收
- 页序、页内 key 顺序、value 字节、compression flag 与旧实现一致。
- 重复 key：batch 内按 timestamp 折叠（较旧不覆盖较新）；与旧页比较时
  `batch.ts >= old.ts` 才覆盖，相等时覆盖，删除项移除。
- 多旧页按页序拼接后与 batch 归并，输出严格升序。
- `cargo nextest run -p fkv_btree`、`-p fkv_task_write` 通过；
  `cargo clippy -p fkv_btree --all-targets -- -D warnings` 零告警。

## DONE（2026-11-04）
实现要点：
- 新增 `MergeVal`（page/batch 两种来源，value 借用）、`OldPageCursor`（跨页游标）、
  `BatchView`（有序视图，乱序时只排下标）、`PageSink`（切页 + overflow 占位）、
  `emit_batch`（同 key 折叠后写出）。
- `merge_data_pages_with_options` 主体改为两路归并循环；删除 `MergedEntry`、
  `merged_value`、`BTreeMap` 导入与整页 clone。
- 语义保持：timestamp 过滤、相等覆盖、delete 移除、页内已有 entry 不二次压缩、
  overflow 估算与指针占位流程原样保留。

测试文件：`crate/core/fkv_btree/tests/btree.rs`
新增用例：`duplicate_batch_keys_keep_newest`、
`merges_multiple_old_pages_in_order`、`delete_at_equal_timestamp_wins`。

验证：
- `cargo nextest run -p fkv_btree` → 16 passed
- `cargo nextest run -p fkv_task_write` → 17 passed（含 `random_batches_match_model`、
  `multi_page_cow`、`write_read_scan`）
- `cargo clippy -p fkv_btree --all-targets -- -D warnings` → 0 warning
- `cargo clippy -p fkv_btree --all-features ...` 被依赖 `fkv_mapping:273`
  既有 `useless_conversion` 阻断（非本次改动）；`fkv_store` 测试被并发修改中的
  `fkv_io_compio` 编译错误阻断（`forget_path` 参数类型不匹配，非本次改动）。

## 复审（2026-11-05 r6）

结论：**通过**。

- `fkv_btree/src/lib.rs:522` `MergeVal`（value 借用）、`:565` `OldPageCursor`、`:608` `BatchView`（乱序只排下标）、`:665` `PageSink`、`:797` `emit_batch`；`:849-884` 两路归并循环直接喂 sink，注释明言「不落中间容器，value 全程借用」。
- `BTreeMap`/`MergedEntry` 在 src 中仅剩 1 处命中（`BatchView` 注释里对旧语义的说明），物化路径已删。
- 测试 `fkv_btree/tests/btree.rs:172`（乱序重复 key 按 ts 折叠取 v3）、`:191`（多旧页三路交错严格升序）、`:214`（相等 ts delete 胜、key 消失）。断言真实覆盖。
