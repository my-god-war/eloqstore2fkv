# rev-43 fkv_btree 未含 leaf 链 / index rebalance / split（仅 merge+seek），doc 注释误导

## 位置
- Rust: `crate/core/fkv_btree/src/lib.rs:1-3`（模块 doc：「B+tree 基础算法：index 下降与 data page **合并 / 分裂**」）。crate 内实际公开：`merge_data_pages_with_options`（`:844`，data 页两路归并 + overflow 回填）、`IndexNode::seek`（`:104`，floor）、`IndexCache`。无 `split`、无 leaf `next/prev` 链维护、无 index 层 rebalance/兄弟合并。树变更编排在 `crate/engine/fkv_task_write/src/lib.rs`（grep `fn split`/`rebalance`/leaf 命中处）。
- C++: `include/write_tree_stack.h` `IndexStackEntry{ idx_page_iter_, handle_, changes_: vector<IndexOp>, is_leaf_index_ }`——index 层下降栈 + 每层待传播的 `IndexOp`（插入/删除子页），分裂/合并/rebalance 由 write task 沿栈递归处理；`MemCachedPage::IsPointingToLeaf()`（`include/storage/mem_cached_page.h:121`）区分 leaf index 与上层 index。

## 现状
`fkv_btree` 只承担**无 IO 的纯算法原语**：data 页 merge、index floor seek、index 页缓存。模块 doc 声称「合并 / 分裂」，但**分裂、兄弟合并、leaf 双向链维护、index 递归 rebalance 都不在本 crate**，而在 engine `fkv_task_write`（与 IO 写、PageId remap、dirty 页记录耦合）。对照审查要点「COW B+tree：合并/分裂、leaf 链、index rebalance、seek/floor」——在 core 树层里，只有「合并」与「seek/floor」两项落地。

## C++ 实现
C++ 把 index 层下降状态压成 `write_tree_stack`（`IndexStackEntry` 栈），沿下降路径记录每层 `changes_`（`IndexOp{key,page_id,op}`）；写提交时自 leaf 向上：页满则 split（新页 + 向父层 push `IndexOp::Insert`）、删除使页低于填充阈值则与兄弟 rebalance 或 merge（向父层 push `IndexOp::Delete` 收回边键），leaf index 与上层 index 用 `is_leaf_index_` 区分。leaf 数据页保持双向链以支撑 scan/seek 顺序遍历。这套 split/merge/rebalance/leaf 链是 B+tree 完整性的核心算法。

## 差异与风险
- 非正确性缺陷：`fkv_task_write` 已实现 split/rebalance/leaf，端到端树可正确工作；`merge_data_pages_with_options` 与 `IndexNode::seek`（floor）已核对与 C++ `add_to_page`/`IndexPageIter::Seek` 语义一致（`tests/btree.rs` `index_seek` 覆盖 floor 与 below-first→leftmost，`tests/seek.rs` 覆盖宽页 seek）。
- 实质问题有二：(1) `fkv_btree` 模块 doc 声称「分裂」但本 crate 无分裂实现，审计者按 crate 判定树层完整性会被误导；(2) split/rebalance/leaf 链算法与 IO/写编排纠缠在 engine，无法在 `fkv_btree` 现有的「无 IO、可单测」风格里回归覆盖——而 C++ 恰恰把这些纯算法与 IO 分离（`write_tree_stack` 是内存栈结构）。边界条件（根分裂增高、单 child 合并收缩、index 页下溢级联）在 core 层缺乏独立单测保障。

## 对齐建议
最小：修正 `fkv_btree` 模块 doc，去掉/限定「分裂」字样并注明分裂·rebalance·leaf 链位于 `fkv_task_write`。建议：把纯算法（node split/merge/rebalance、leaf 双向链维护、index 栈传播）下沉进 `fkv_btree`，以内存页 + PageId 抽象为入参、返回变更集，使这些路径可在无 IO 环境单测，对齐 C++ `write_tree_stack` 的职责划分。

## 严重度
P2

## 核验记录
- 通读 `fkv_btree/src/lib.rs` 全文：公开项仅 merge/seek/IndexCache/decompress，无 split/rebalance/leaf 链函数。
- grep：`fn split`/`rebalance`/`leaf` 在 core 仅命中 `fkv_btree` 的测试与注释（`shared_ledger_splits`、leaf index 测试页），真正实现命中 `engine/fkv_task_write`、`fkv_task_reopen`、`fkv_shard/backend`。
- C++ `write_tree_stack.h` 确认 index 下降栈 + `IndexOp` 传播模型；`mem_cached_page.h:121 IsPointingToLeaf` 存在。结论：merge+seek 已实现且对齐；split/rebalance/leaf 链未在 core 树 crate 实现（在 engine），且 core doc 描述与实现不符。

## 甄别（2026-11-05）

结论：**拒绝**。事实陈述大体准确（split/rebalance/leaf 链确在 `fkv_task_write`
而非 `fkv_btree`），但两条实质诉求的前提均不成立，且与已决策契约冲突：

1. **「doc 误导」不成立**。`fkv_btree/src/lib.rs:1` 写的是「**data page** 合并 /
   分裂」，该语义由 `merge_data_pages_with_options`（`:849`）+ `PageSink` 落地：
   `push`（`:741-756`）页满即切页，归并结果重切为多页（fmt-19/22 四分页重分配的
   载体）——data 页级「分裂」就在本 crate。树级 split/rebalance/leaf 链的位置
   `lib.rs:3` 已明确注明：「完整的 COW 写路径在 `fkv_task_write` 里编排」。
   意见把「分裂」读作树级节点分裂后判 doc 失实，属误读。
2. **「C++ 把纯算法与 IO 分离」对照前提不实**。`write_tree_stack.h` 全文 33 行，
   只是 `IndexStackEntry{iter, handle, changes_, is_leaf_index_}` 状态结构，
   不含任何 split/rebalance 算法。C++ 树级 split / 1/4 重分配 / leaf 链维护实际
   在 `src/tasks/batch_write_task.cpp`（`FinishIndexPage`/`Redistribute`
   `:982-1019`、`SetNextPageId` `:135-179`），与 `AllocPage()` / `WritePage` /
   `LoadTripleElement` 直接交织——**同样是 task 层 + IO 耦合**，与 Rust
   `fkv_task_write` 的层次划分一致，不存在意见声称的 C++ core 层纯算法。
3. **「无法无 IO 单测」不实**。`fkv_task_write/src/lib.rs` 含 18 个 crate 内单测，
   直接覆盖本意见点名的算法：`index_page_quarter_redistribution_balances_tail`
   （`:2207`）、`split_lens_matches_encoded_page_lengths`（`:2425`）、
   `pick_split_matches_bruteforce_probe`（`:2507`）、
   `redistribution_migrates_overflow_slot`（`:2524`）等；eng-53（pick_split +
   overflow 迁移 + 空表首写重平衡，4 测试）已经 r6 store/write 复审收编
   （`reviewed/00-r6-store-write.md:21`）。
4. **层次位置是既定决策**。eng-65a（r6 复审 DONE）明确扫描遍历契约为叶子 next
   链、「链一致性由**写路径**负责」；sto-67（索引页 1/4 重分配）、fmt-19
   （重分配分隔键）的实现位置均在 `fkv_task_write` 并经复审接受。把算法下沉重构
   进 `fkv_btree` 无正确性、无可测性收益，违反仓规「避免过度设计」。
5. 意见自评「非正确性缺陷」（差异与风险节首句），与甄别一致；剩余诉求仅剩
   分层审美，不立项。

### 去向

`issue/reject/rev-43-btree-split-rebalance-leaf-chain-not-in-core.md`。
