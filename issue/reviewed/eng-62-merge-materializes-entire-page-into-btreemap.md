# eng-62 merge 先把整页物化进 BTreeMap（key/value 全量 clone、key 双份），C++ 双指针流式合并不落中间容器

## 位置
- Rust: `crate/core/fkv_btree/src/lib.rs:354-393`（`merge_data_pages_with_options`：旧页每条 entry `iter.key().to_vec()` **两次**（一次作 map key、一次在 BatchEntry 内）+ `iter.value().to_vec()`；batch entry 再 `entry.clone()`；之后逐条取出喂 builder）
- C++: `src/tasks/batch_write_task.cpp` `ApplyOnePage` 合并循环（约 `:530-700`）：`base_page_iter` 与 `change_it` 双指针流式推进，`std::string_view` 零拷贝直送 `add_to_page`，无中间容器

## 现状
每次 COW 重写一个数据页，额外付出约 1× 页量级的 key/value 堆拷贝 + 数十条 BTreeMap 节点分配/释放（key 在 map key 与 BatchEntry 中重复存两份）。

## C++ 实现
旧页迭代器与已排序 batch 双路归并，比较即可决出先后，零中间分配。

## 差异与风险
写路径 p99 常数项放大（alloc/free 占比高）；无字节差异，纯性能。

## 对齐建议
- 改两路归并：`DataPageIter`（旧页）+ 已排序 batch 切片双指针直喂 `DataPageBuilder`；
- 过渡方案：至少去掉 key 双份（map key 借用或 entry 里存引用下标）、`from_page` entry 的 value 用 Cow/借用。

## 性能优化建议
单页重写从「O(entries) 次分配 + 2× 拷贝」降到 0 中间分配；批量写吞吐与尾延迟直接受益（与 eng-49 阶段 2 的「合并重建」正交）。

## 严重度
P1（写路径核心常数项）

## 核验记录（2026-11-04）
Rust 物化路径与 key 双 clone 实读确认（`:355-373` 旧页、`:375-393` batch）；C++ 流式合并引用自子代理摘录。

## 审查结论（2026-11-04）

结论：**成立，已修复**。

逐条核验：

1. 位置：`merge_data_pages_with_options` 实际在 `crate/core/fkv_btree/src/lib.rs:346-488`
   （issue 写 `354-393`，行号漂移但函数正确）。
2. 现状属实：旧页每条 entry `iter.key().to_vec()` 两次 + `iter.value().to_vec()`，
   batch entry 整体 `clone()`，最后从 `BTreeMap` 逐条取出喂 builder。
3. C++ 对照：本仓库 `cpp/eloqstore` 子模块未初始化，无法本地复核
   `batch_write_task.cpp` 的双指针实现；但“旧页有序 + batch 有序、可直接归并”
   由 Rust 调用方保证，方向成立。
4. 附带发现：`batch_li` 在 `fkv_task_write::write_batch` 中已按 key 排序去重；
   `merge_data_pages` 公开 API 仍可能收到乱序 batch（现测试 `merge_and_delete`
   即是），因此流式实现保留“乱序时只排下标的稳定排序”兜底，语义不变。
5. 无重复 issue：`eng-49` 只做 index 页重建合并，与本条 data 页 entry 合并正交。

已按建议改为双指针流式合并，页内字节与既有测试语义保持不变；详见
`issue/todo/eng-62a-streaming-merge.md`。

## 复审（2026-11-05 r6b）

通过。

- `merge_data_pages_with_options`（`crate/core/fkv_btree/src/lib.rs:849`）现为 `OldPageCursor` + `BatchView` 双游标两路归并直喂 `PageSink`，注释明写「不落中间容器；value 全程借用，压缩值才分配」；`Ordering::Equal` 分支按 timestamp 新旧取舍、相等 batch 覆盖，语义对齐旧实现。BTreeMap 物化路径不存在。
- 实施件 `eng-62a-streaming-merge.md` 已在 `issue/reviewed/`。
