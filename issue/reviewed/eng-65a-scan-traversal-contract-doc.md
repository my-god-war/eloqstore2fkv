# eng-65a 记录扫描遍历来源契约偏离（叶子 next 链 vs C++ index 栈）

## 位置
- `doc/migration.md` §8.1「已知差异（不影响双向互读）」
- `crate/engine/fkv_task_scan/src/lib.rs:1`（模块文档）

## 方案
1. §8.1 增加一行：Rust 扫描遍历顺序来自 data page `next_page_id` 单向链表；
   C++ `ScanTask::AdvanceToNextLeaf` 沿 `index_stack_` 推导，`NextPageId` 仅写路径维护。
   写明风险面：扫描正确性依赖写路径（页重分配 / reopen / TTL 重写 / rebalance）
   维持 next 链；链陈旧会串页/跳页。写明缓解：eng-65b 的环检测；
   预取深度限制见 eng-42/52。
2. 模块头注释补一句：遍历来源为叶子链（与 C++ index 栈不同），链一致性由写路径负责。

## 验收
- `doc/migration.md` 差异表含该条，措辞与既有行一致。
- 模块注释说明遍历来源契约，不引入行为改动。

## DONE（2026-11-04）

### 实现要点
- `doc/migration.md` §8.1 新增「扫描遍历来源是叶子 next 链」一行：写明 Rust 与
  C++ `ScanTask::AdvanceToNextLeaf`（index 栈）的差异、对写路径维护链的依赖、
  链陈旧后果，以及环检测 / eng-42 / eng-52 的缓解。
- `crate/engine/fkv_task_scan/src/lib.rs` 模块文档补充遍历来源契约与预取 1 页的原因。

### 验证
- 纯文档 + 注释，无行为改动；`cargo fmt -p fkv_task_scan -- --check` 通过。

## 复审（2026-11-05 r6）

通过。`doc/migration.md:218` 存在「扫描遍历来源是叶子 next 链」条目，写明与 C++
`ScanTask::AdvanceToNextLeaf`（index 栈）的差异、写路径维护依赖与 eng-65b 环检测缓解；
`fkv_task_scan/src/lib.rs:1-5` 模块文档记录同一契约。纯文档/注释，无行为改动。
