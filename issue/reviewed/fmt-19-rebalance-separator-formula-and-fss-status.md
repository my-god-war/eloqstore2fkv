# fmt-19 四分页重平衡后的分隔键公式与 C++ 不同；顺带更新 eng-49 中 find_shortest_separator 的实现状态

## 位置
- Rust: `crate/engine/fkv_task_write/src/lib.rs:1036-1048`（`page_separators`：首段沿用父 index 原分隔键，`idx >= 1` 用 `find_shortest_separator`）；`:1064-1077`（`find_shortest_separator` 已实现，语义对齐 C++：right 公共前缀+1）；`:681`（`rebalance_data_pages` 触发的重分配也走 `page_separators(&page_li, sep)?` → 用最短分隔键）
- C++: `src/tasks/batch_write_task.cpp:1086-1095`（`Redistribute` 触发时 `page_key.assign(iter.Key())` —— 用**新页真实首键**覆盖分隔键）；对照分裂路径 `:506-510` 用 `FindShortestSeparator`、首段 `:451` 沿用父分隔键

## 现状（eng-49 状态更新）
eng-49 审查结论（2026-11-04）时 `find_shortest_separator` 尚未实现；当前代码已落地且三个使用点（数据页分裂、首段沿用、index 层原始键 `:1175`）与 C++ 对齐。**残余差异仅一处**：重平衡改变页首键时，Rust 用最短分隔键，C++ 用真实首键。

## 差异与风险
两者都落在合法区间 `(prev_last, cur_first]`，仅分隔键长度不同：
- 盘面字节级 fixture 差分会捕获该差异（影响 issue 01 验收）；
- index 页空间利用率略有差别（最短分隔键更省）。

## 对齐建议
- 若以 C++ 盘面为 oracle：rebalance 改变页首键时改用真实首键；
- 若以「更优」为由保留最短分隔键：在 fixture 差分中把该点列为已知允许差异，并记入 doc/migration.md。
- 不要移植 C++ 的两个死接口：mutating `FindShortestSeparator`（`comparator.cpp:39-66`）与 `FindShortSuccessor`（`:90-105`）全仓零调用。

## 严重度
P2（仅影响字节级对齐验收，不影响正确性）

## 核验记录（2026-11-04）
`find_shortest_separator` 现存实现、`page_separators` 调用点、rebalance 路径共用该函数均实读确认；C++ `page_key.assign(iter.Key())` 实读确认（`batch_write_task.cpp:1086-1095`）。

## 审查结论（2026-11-04）
有道理，采纳。核对结论：

1. `find_shortest_separator` 已落地，且与 C++ 非互斥版 `FindShortestSeparator(left,right)` 逐字节等价（取 right 公共前缀+1，`comparator.cpp:78-95` 实读）。三个使用点（首段沿用父分隔键、数据页分裂、index 层原始键）与 C++ 对齐，无需改动。
2. 残余差异属实且唯一：数据页四分页重分配（C++ `FinishDataPage` 的 `Redistribute` 分支，`batch_write_task.cpp:1082-1090`）用 `page_key.assign(iter.Key())` 把新页分隔键改成**真实首键**；Rust 走 `page_separators` 用 `find_shortest_separator`。C++ 的互斥版 `FindShortestSeparator` 与 `FindShortSuccessor` 全仓零调用，不移植（同意 issue）。
3. 以 C++ 盘面为 oracle（`COMPAT_BASELINE` 为 L1 兼容基线）→ 采纳「rebalance 改变页首键时改用真实首键」方案，而非记入已知差异。
4. 语义边界：仅**新页**（重分配对里 idx 较大、首键被改写的那页）需改真实首键；前一页首键不变，其父分隔键仍按最短分隔键保持不变，与 C++ 一致。

子项：单一子项，无需拆分。

## DONE（2026-11-04）
实现要点：
- `rebalance_data_pages` 改为返回与 `page_li` 等长的 `Vec<bool>` 标记：被重分配替换的新页（idx）置真。
- `page_separators` 新增 `redistributed_li` 参数：标记页分隔键改用真实首键，对齐 C++ `page_key.assign(iter.Key())`；其余页仍用 `find_shortest_separator`。合并遍历，未新增额外循环。

改动文件：`crate/engine/fkv_task_write/src/lib.rs`

测试命令与结果：
- `cargo nextest run -p fkv_task_write` → 18 passed, 0 failed（含新增 `redistributed_page_uses_real_first_key`）
- `cargo nextest run -p fkv_store` → 69 passed, 0 failed（集成回归）
- `cargo clippy -p fkv_task_write --no-deps -- -D warnings` → 零警告

## 复审（2026-11-05 r6）

通过。`fkv_task_write/src/lib.rs:786/:833` `redistributed_li` 贯通，`page_separators` :1405-1419 对标记页改用真实首键；测试 `redistributed_page_uses_real_first_key`（src/lib.rs:2322-2333，断言 idx=1 用 `b"k05"` 真实首键 :2331、未标记页仍最短分隔键 :2332）。实现落点在 task_write（write lane），本轮核对其行为契约。
