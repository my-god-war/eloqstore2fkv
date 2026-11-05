# eng-68 changed_page_li 在 compact/truncate 后无人取出：跨批次无界累积 + 下一次写误逐大批 IndexCache 热条目

## 位置
- Rust: `crate/core/fkv_mapping/src/lib.rs:580-581`（`take_changed_page_li` 唯一取出接口）、`:615,636,653`（`remap_page`/`free_page`/等每调用 append 一条，无去重无上限）；`crate/engine/fkv_store/src/lib.rs:359-377`（`invalidate_index_cache` 是全仓唯一消费点，`:948,1570` 两处调用）；`:1763-1806`（`compact_file` 只 begin/commit/append_mapping_log，**不调用** `invalidate_index_cache` 也不 take）；`:1302-1353`（`truncate` 全表 free_page 后只 `clear_index_cache`，同样不清 `changed_page_li`）
- C++: `src/storage/page_mapper.cpp`（PageMapper **不持有** dirty/changed 列表，变更跟踪只在写任务生命周期内）；`include/storage/page_manager.h:104-108`（`TryRecycleCachedPage`：写后即时回收被 CoW 孤立的缓存槽）

## 现状
1. 纯维护循环（shard 写后 `maintain_after_write` → `compact_file`，`fkv_shard/src/lib.rs:351-357`）或 truncate 后无写时，`changed_page_li` 按每次 remap 追加，无界增长（百万页表约 4 MB/轮，重复搬迁重复入列）；
2. 累积的陈旧条目在**下一次 put** 触发的 `invalidate_index_cache` 中被当作本批变更 → 内容未变（仅物理搬迁）的热点 index 页成批逐出 → compaction 后紧跟一次小写即缓存雪崩。

## C++ 实现
变更跟踪生命周期 = 单次写任务；缓存回收与 CoW 同步发生，不存在跨任务累积。

## 差异与风险
内存缓慢增长 + compaction 后缓存命中率骤降（重新逐页读盘）。

## 对齐建议
- `compact_file` / `truncate` 结束时显式 `take_changed_page_li` 丢弃（物理搬迁不改逻辑页内容，index 缓存键是逻辑 PageId，无需失效）；
- `changed_page_li` 改 batch 截断语义（与 `dirty_li` 的 `clear_dirty_page_li` 对称），公开 API 出口处 `debug_assert!` 已清空。

## 严重度
P1（缓存雪崩 + 无界内存）

## 核验记录（2026-11-04）
`take_changed_page_li` 唯一消费点、`compact_file`/`truncate` 无 take、push 点三处均实读/全仓 grep 确认；C++ 无 changed 列表引用自子代理摘录。

## 审查结论（2026-11-04）

**合理，采纳。** 复核结果：

- `crate/core/fkv_mapping/src/lib.rs:614,635,652` 三处 push 确认（`remap_page` / `set_file_page` / `free_page`）。
- `crate/engine/fkv_store/src/lib.rs` 中 `invalidate_index_cache` 是唯一 `take_changed_page_li` 消费点，仅 `put` 与 `clean_expired` 调用；`compact_file`、`truncate` 均不取，确认跨批次累积。
- `IndexCache`（`crate/core/fkv_btree/src/lib.rs:135`）键为逻辑 `PageId`，`remove(page_id)`/`clear()`；compact 只换物理 `FilePageId`，缓存内容仍有效，无需逐页失效 → 累积条目在下一批被误逐属实。
- 额外发现：`abort_batch` 只按 `dirty_len`/`garbage_len` 回滚，未截断 `changed_page_li`，失败的写/compact 批次同样残留，一并修。

**修正点**：issue 建议「take 后丢弃」改为新增 `clear_changed_page_li()`（保留堆容量，与 `clear_dirty_page_li` 对称）；`take_changed_page_li` 仍服务写路径精确失效。

已拆分为：
- `eng-68a-changed-page-li-batch-rollback-and-clear.md`
- `eng-68b-compact-truncate-discard-changed-and-guard.md`

## DONE（2026-11-04）

`eng-68a` / `eng-68b` 均已完成：`abort_batch` 按 journal 长度回滚 changed、
`compact_file` / `truncate` 出口清空 changed、批出口 debug 断言。父 issue 归档。

## 复审（2026-11-05 r6）

通过（父条目）。eng-68a / eng-68b 均已按当前源码复核，见各自 r6 复审小节。
