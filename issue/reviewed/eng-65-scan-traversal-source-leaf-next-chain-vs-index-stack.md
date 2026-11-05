# eng-65 扫描叶子推进信任 data page next 单向链表，C++ 用 index 栈推导顺序（读路径不依赖 next 链）

## 位置
- Rust: `crate/engine/fkv_task_scan/src/lib.rs:181-189,272`（`AdvanceToNextLeaf`：`DataPage::new(&page)?.next_page_id()?` —— 遍历顺序来源 = 叶子链；`swap(&mut page, &mut spare)` 复用缓冲）
- C++: `src/tasks/scan_task.cpp:117-155`（`AdvanceToNextLeaf` 沿 `index_stack_` 逐层推进）；`scan_task.h:41-66`（持有 `MemCachedPage::Handle`）；全仓 `NextPageId()` 仅 `batch_write_task.cpp:135-179` 写路径维护链表时使用，读路径仅 Floor 用 `PrevPageId`（`read_task.cpp:229`）

## 现状
fkv 扫描正确性额外依赖写路径（页重分配、reopen、TTL 清理重写、rebalance）对 next 链的维护；链陈旧时静默串页/跳页/读到已复用 PageId。

## C++ 实现
顺序完全由 index 页 + mapping 快照推导，next 链只是写路径的内部维护结构，不暴露给读路径。

## 差异与风险
差异本身确定；实际风险取决于写路径维护正确性（当前测试未发现坏例）。预取深度受限这一点已由 eng-42/07/eng-52 记录，本条仅指**遍历来源契约**。

## 对齐建议
1. 在 `doc/migration.md` 记录该契约差异为已知偏离；
2. 可选：提供 index 栈式推进路径（同时解除预取深度限制）；或 reopen/rebalance 后加链一致性 debug 校验（debug_assert：叶子链顺序 == index 推导顺序）。

## 严重度
P2（契约记录 + 防御校验；非当前正确性事故）

## 核验记录（2026-11-04）
Rust `next_page_id` 推进实读确认；C++ index 栈推进与 NextPageId 调用面引用自子代理摘录（含 grep 结论）。

## 审查结论（2026-11-04）

核验通过，判定合理，但按 issue 自身 P2 定位只落地「契约记录 + O(1) 防御校验」，
不重写扫描器、不新增 index 栈式推进路径。

- 行号核实：`crate/engine/fkv_task_scan/src/lib.rs:181`（`next_page_id` 取值）、
  `:272`（`swap` 换页）与 issue 一致；`next_page_id` 仅写路径与扫描消费，
  读路径其余入口不用，差异确凿。
- C++ 侧源码（`cpp/eloqstore`）当前为空目录，`src/tasks/scan_task.cpp` 无法二次核验，
  沿用 issue 的核验记录与 COMPAT_BASELINE 描述，不额外展开。
- 不做 index 栈推导：需要新建整棵索引的叶子序号推导路径，属功能级改造，
  与「最小改动」及 eng-42/52 已记录的预取深度议题重叠。
- 防御校验取「叶子链环检测」：环会让当前实现无限循环（比串页更难排查），
  以 `PageMapper` 逻辑页槽位数为 O(1) 上界，超过即 `Corrupted`。
  链陈旧导致的串页/跳页仍按契约记录，不在本条修。

拆分：
- `eng-65a`：`doc/migration.md` + 模块注释记录遍历来源契约偏离。
- `eng-65b`：扫描叶子链环检测 + 测试。

## DONE（2026-11-04）

按 P2 定位落地「契约记录 + 防御校验」，未重写扫描器、未新增 index 栈推进路径。

- `eng-65a`：`doc/migration.md` §8.1 + 模块注释记录遍历来源契约偏离。
- `eng-65b`：叶子链环检测（`PageMapper::page_cnt()` 上界 + 自环判定）+ 测试。
- 验证：`cargo nextest run -p fkv_store -p fkv_task_scan` 72 passed；
  `clippy -D warnings` 零警告；`fmt --check` 通过。
- 遗留：链陈旧导致的串页/跳页仍依赖写路径维护，未做 index 推导一致性校验
  （需功能级 index 栈改造，超出本条范围）。

## 复审（2026-11-05 r6）

通过。本文件为父条目（契约记录 + 防御校验，P2 定位），两项均已落地并复核：
`doc/migration.md:218`（§8.1 扫描遍历来源差异行）与 `fkv_task_scan/src/lib.rs:184`（环检测）。
详见 `eng-65a` / `eng-65b` 的 r6 复审。
