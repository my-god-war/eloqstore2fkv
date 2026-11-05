# rev-01 主写路径的 batch 事务边界只覆盖 page mapper，不覆盖 segment_mapper

## 位置

- Rust 写路径：
  - `crate/engine/fkv_store/src/lib.rs:822`（`put_at`）→ `:835` `mapper.begin_batch()`、`:839` `commit_batch()`、`:841` `abort_batch()`：**只对 `mapper_map` 开批次**。
  - `put_at_inner` 在 `:901` 与 `:922` 两次 `write_batch` 都传 `segment_mapper: reborrow_segment(&mut segment_mapper)`（`:915`、`:936`）。
  - `clean_expired` 同样：`:1536` 只 `mapper.begin_batch()`，两次 `write_batch` 都传 `segment_mapper`（`:1552`、`:1570`）。
- 反证（正确写法）：`put_large` 同时给两个 mapper 开批次 —— `:1840-1844` `segment_mapper_map…begin_batch()`，`:1893-1898` 按结果 `commit/abort`。

## 现状

`write_batch` 在遇到「被覆盖/删除的 entry 原本是 large value」时会通过 `segment_mapper` 释放旧 segment 页（`free_large_value` / `free_replaced_overflow` 一路），也可能为大 value 分配新 segment。

但 `put_at`（以及 `clean_expired`）只把 **主 page mapper** 放进 `begin_batch/abort_batch` 的回滚日志（`MappingTbl` 的 journal 见 `crate/core/fkv_mapping/src/lib.rs:504-539`）。`segment_mapper` 没有任何 journal。

## 差异与风险

中途失败时（OOM / IO 失败 / 分支区间越界 `OutOfSpace`）`abort_batch` 只回滚主 mapper：

- 主树的删除/覆盖被回滚，entry 恢复；
- 但 `segment_mapper` 里已执行的 `free_page` **不回滚** → 该 entry 的 segment 页已被释放并可能被下一次分配复用 → **live key 指向已被回收的 segment**（悬垂读，读到别的值或零）；
- 若本次分配了新 segment（覆盖旧值），那些新 segment 页在失败后**泄漏**（无回滚，也无人再引用）。

`put_large` 已正确处理（两个 mapper 都开批次），说明这是 `put_at` / `clean_expired` 的遗漏，而非有意设计。

## C++ 对照

C++ `WriteTask::Abort` 回滚的范围是整个写事务：`RollbackBranchFileTail`、清 free-file-page 列表、`IoMgr()->AbortWrite`（`src/tasks/write_task.cpp:204-236`），data 与 segment 分配器在同一事务里，不存在「data 回滚、segment 不回滚」的分裂。

## 对齐建议

1. `put_at` / `clean_expired` 与 `put_large` 一致：对 `mapper_map` 与 `segment_mapper_map` 同时 `begin_batch`，按同一结果 `commit`/`abort`。
2. 优先在一个 helper 里封装「双 mapper 批次」（如 `with_batch(tbl, f)`），避免第三条路径再漏。
3. 补失败注入测试：large value 被小值覆盖 / 被删除时，在第二次 `write_batch` 注入错误，断言 abort 后 segment 页未释放、且原 large value 仍可读。

## 性能优化建议

无（正确性项）。

## 严重度

P1（静默数据损坏窗口；触发需批中途失败 + 批含 large value 的删除/覆盖）

## 核验记录

- 本人实读：`fkv_store/src/lib.rs` 的 `put_at`(822-845)、`put_at_inner`(846-960)、`clean_expired`(1525-1589)、`put_large`(1831-1899)；`fkv_mapping/src/lib.rs` 的 `begin_batch/commit_batch/abort_batch`(504-539)。
- segment 释放点已确认在 `write_batch` 内部：`crate/engine/fkv_task_write/src/lib.rs:576-580`（`free_overflow_chain` + `fkv_large::free_value(segment_mapper, …)`），`write_batch` 经 `WriteArgs.segment_mapper`(46) 拿到该 mapper。即失败回滚只覆盖 `mapper` 时，这里的 `segment_mapper` 改动不会回滚。

## 审查结论（2026-11-04）

- **成立（部分已修复）**：issue 记录的 `put_at` 遗漏属实，行号以当前代码为准。
- 已不成立的部分：`clean_expired` 与 `put_large` 均已在 core-25 / eng-73 中把
  `segment_mapper` 纳入 `begin_mapper_batch` / `settle_mapper_batch`（或显式
  `segment_mapper.begin_batch/commit/abort`），无需再改。
- 仍成立：`put_at` 只对主 mapper 做 `begin_batch/commit_batch/abort_batch`；
  `put_at_inner` 的两次 `write_batch` 都传 `segment_mapper`，大 value 被覆盖/删除时
  `fkv_large::free_value` 的释放不会回滚。
- 附带发现（本次一并修正）：`put_at_inner` 在 `defer_ttl == false` 时内联调用
  `maybe_clean_expired`，而 `clean_expired` 自带 mapper 批次；`begin_batch` 幂等导致
  内外批次共享同一 journal，内层 `settle` 会提前关掉外层 journal。改为在
  `put_at` 结算批次之后再执行 TTL 维护。
- 拆分 1 个 todo：`rev-01a-put-at-dual-mapper-batch.md`。

## 复审（2026-11-05 r6）

通过（拆分父条目）。`put_at` 遗漏已由 rev-01a 修复并复核（`fkv_store/src/lib.rs:1332-1334` begin_mapper_batch/settle_mapper_batch）；clean_expired/put_large 批次经核心断言路径同样收敛。
