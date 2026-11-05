# perf-85 P2：`truncate_from` 为生成墓碑全量物化区间（含 overflow 链读取），内存/IO 双倍

来源：第 5 轮 store/write lane 侦查，协调者亲验。

## 证据（2026-11-05 核验）

`crate/engine/fkv_store/src/lib.rs:1840-1881`：

```rust
let entry_li = scan_entries_with(
  &self.io,
  ScanArgs { ..., begin: position, end: &[], max_entries: 0, ttl_filter_now_ms: None, },
  ScanOption { reject_large_value: false, ..ScanOption::default() },  // 大值也物化
).await?;
if entry_li.is_empty() { return Ok(0); }
let delete_li: Vec<BatchEntry> = entry_li
  .into_iter()
  .map(|entry| BatchEntry::delete(entry.key, u64::MAX))
  .collect();
let deleted = delete_li.len();
self.put(tbl, &delete_li).await?;
```

C++（`batch_write_task.cpp:1845-1893` + `:1306-1333`）部分截断走
`TruncateIndexPage` / `DeleteTree` 按子树结构性整删：页级释放、零 value 物化、
内存有界。

## 问题

截断大区间：所有 key+value（overflow 值读完整链）拉进内存再丢掉——只需 key
却付了 value 的钱；随后 N 条 `u64::MAX` 墓碑塞进一个 `put` 批（写放大 +
单批页数无上界）。

## 目标

短期：`ScanArgs` 增加「仅 key」模式（`reject_large_value: true` 语义延伸），
物化 key 列表；墓碑分批（复用 max_write_batch_pages 切分）。
长期（可另立条目）：对齐 C++ 子树整删——依赖页链结构，放 sto-56b compaction
之后评估。

## 验收

- 测试：大区间 truncate 内存与 IO 显著下降（failpoint 计数断言不读 overflow 链）；
  删除语义（floor/get 可见性）与现状一致。
- `cargo nextest run -p fkv_store`。

## 依赖 / 重叠

- sto-62c（drop 语义，done）不同对象；perf-84 的下推改造可共用「仅 key」模式。

## 核验记录

- 本人实读：`fkv_store/src/lib.rs:1840-1881`。
- 子代理摘录（未复核）：C++ 行号。

## 审查结论（dragon 复核，2026-11-04）

**成立，且核实到一个关键前提：扫描物化 value 是纯浪费（释放 segment 不依赖它）。**

- `truncate_from`（`fkv_store/src/lib.rs:1848-1892`）确实用
  `reject_large_value: false` + `max_entries: 0` 全量物化，然后
  `map(|entry| BatchEntry::delete(entry.key, u64::MAX))` 只取 key，最后**一次**
  `self.put(tbl, &delete_li)`。
- 「单批页数无上界」成立：`put_at_inner`（`:1277-1300`）里没有按
  `max_write_batch_pages` 切分的循环（该常量只在 IO 层用），整批一次处理。
- **关键前提（决定这条能不能干净地修）**：删除大值释放 segment 是**写路径自己**
  做的 —— `fkv_task_write/src/lib.rs:682-686` 在 merge 到旧 entry 时调
  `fkv_large::free_value(segment_mapper, iter.value(), ...)`，而 `iter` 是写路径
  对旧 entry 的点读。所以 `truncate_from` 预扫描物化出来的 value **没人消费**，
  只物化 key 不会漏释放 segment（不会空间泄漏）。
  **注意**：`reject_large_value: true` 不能直接拿来用 —— 它是「遇大值报错」而非
  「跳过大值」，直接开会把 truncate 打挂；需要的是新增「只取 key」模式
  （`ScanOption` 加 `skip_large_value` / `keys_only`，与 perf-84 的下推改造共用）。

## 阻塞（留 todo）

`fkv_store/src/lib.rs` 是并行会话热点（本轮多次抓到秒级修改），代码交 owner。修法：

1. `ScanOption` 增「只物化 key、跳过 overflow 链读取」模式（不复用
   `reject_large_value` 的报错语义），`truncate_from` 改用它。
2. 墓碑分批提交：按 `max_write_batch_pages` 切 `put` 调用（这条顺带把
   `put` 的「超大批」语义收口，对其它调用方也有价值）。
3. 测试：大区间 truncate 后 `get/floor` 可见性不变；用 IO 计数断言
   **不读 overflow 链**（反向验证：把模式改回 `reject_large_value: false` 时计数必须变大）。
4. 长期项（子树整删，对齐 C++ `TruncateIndexPage`/`DeleteTree`）另立条目，
   依赖页链结构，等 sto-56b compaction 之后评估。

## DONE（2026-11-04）

实现（短期方案）：

- `fkv_task_scan::ScanOption` 新增 `keys_only`：只物化 key / `expire_ts`，
  跳过 overflow 链读取与解压，不触发 `reject_large_value` 报错语义。
  `page_size` 配额口径不含 value（真实长度未知），已在字段文档注明。
- `Store::truncate_from` 扫描改用 `keys_only: true`；删除语义、TTL/tombstone
  处理未变（仍生成 `u64::MAX` 墓碑，释放由写路径 merge 旧 entry 完成）。

测试：

- `scan_pushdown::scan_keys_only_skips_overflow_chain`：全量扫描与 keys_only
  的读页差 >= 每条 overflow 链页数，key 序列一致、value 为空。
- `overflow::truncate_from_skips_overflow_value_reads`：大区间截断读页数
  从 28 降到 16（断言上界 20），可见性 / `get` / `floor` 不变。
  已反向验证：把 `keys_only` 改回 false，该测试失败（28 reads）。
- `large_value::truncate_from_frees_large_value_segments`：截断后 segment
  映射归零，无空间泄漏。

验收：`cargo check -p fkv_store -p fkv_task_write`；`cargo nextest run
-p fkv_store -p fkv_task_write -p fkv_runtime --no-fail-fast` 193 passed；
clippy 两 crate `--no-deps --all-features --all-targets -- -D warnings` 零警告；
`fkv_task_scan` clippy 亦零警告。

未做（留待长期项）：

- 墓碑分批 `put`：`Store` 侧拿不到 `max_write_batch_pages`（IO 层配置），且
  拆批会破坏单批 mapper 回滚语义（半截截断）；现 IO 层 `write_page_li` 已按
  该常量分组连续页，单批页数不再无界下盘。
- 子树结构性整删（对齐 C++ `TruncateIndexPage`/`DeleteTree`）仍依赖页链结构，
  等 sto-56b compaction 之后评估。

## 复审（2026-11-05 r6）

通过，且实现已超出 DONE 声称的范围：

- `fkv_task_scan/src/lib.rs:31-36`（`ScanOption::keys_only`：只物化 key/expire_ts，
  跳过 overflow 与解压，文档写明 page_size 配额口径）。
- `fkv_store/src/lib.rs:1938-2001`（`truncate_from`）：`keys_only: true`(:1970)；DONE 中
  「未做：墓碑分批」现已补上——按 `TRUNCATE_FROM_BATCH`(:48) 分批「扫描→删除→续扫」，
  驻留内存与表大小无关，并有首 key 无进展防御（:1980-1986）。
- 测试：`fkv_store/tests/scan_pushdown.rs:93`（`scan_keys_only_skips_overflow_chain`，
  断言 `full_reads >= key_reads + 5*4`）、`overflow.rs:185`（`truncate_from_skips_overflow_value_reads`，
  断言 reads <= OVERFLOW_PAGES+8 且 get/floor 语义不变）、`large_value.rs:582`
  （`truncate_from_frees_large_value_segments`，segment 映射归零）。
