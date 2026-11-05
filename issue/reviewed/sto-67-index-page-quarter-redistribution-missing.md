# sto-67 P2：索引页缺四分之一重分配；且该缺口的追踪已断链（core-15 reject ↔ eng-53 defer 互踢）

来源：第 5 轮 store/write lane 侦查，协调者亲验（断链事实）。

## 证据

Rust 数据页有 rebalance、**索引页没有**：
- 数据页（存在，正确）：`crate/engine/fkv_task_write/src/lib.rs:1055-1079`
  `rebalance_data_pages`，quarter/three_quarter/restart_num>1 三条件齐备。
- 索引页（缺失）：`fkv_task_write/src/lib.rs:1297-1402` `build_index_pages` /
  `build_index_page_li` 只有「放不下就 finish 新页」：

```rust
for child in &children[1..] {
  if builder.add(&child.first_key, child.page_id)? { continue; }
  out_li.push((page_first_key, builder.finish()?));
```

C++（`batch_write_task.cpp:990-1003` `FinishIndexPage`）：

```cpp
const uint16_t one_quarter = Option()->data_page_size >> 2;
if (cur_page_len < one_quarter && prev_handle &&
    prev_handle->RestartNum() > 1 && prev_handle->ContentLength() > three_quarter)
{
  page_view = Redistribute(prev_handle, page_view, cur_page_key);   // 索引版 :1189-1265
}
```

**断链事实**：`issue/new/00-index.md:99` 记 `core-15` 已 reject（判与 eng-53
重复），而 eng-53 的结论文档把该项 defer 给 core-15 后关闭——两条都已关闭，
当前没有任何 open issue 跟踪索引页重分配。属「已知缺口掉出清单」。

## 问题

索引页分裂可产生 < 1/4 的稀疏尾页：大索引树页数、树高、盘面布局与 C++ 不一致
（oracle 差分），fanout 与空间利用变差。

## 目标

`build_index_page_li` 增加 `rebalance_data_pages` 的索引版：尾页不足 1/4 且
前页超 3/4 且 restart_num > 1 时，按 restart 边界重分配（fmt-19 的最短分隔键
公式沿用）。数据页与索引页两份 rebalance 的公共骨架抽 helper。

## 验收

- 测试：构造索引页分裂序列，断言无 < 1/4 尾页（restart 边界允许的最近值）；
  与 C++ 同输入的页数/分隔键对账（oracle 或手造 fixture）。
- `cargo nextest run -p fkv_task_write`。

## 依赖 / 重叠

- 显式接替断链的 core-15/eng-53 遗留；fmt-19（done）分隔键公式是子任务依赖。

## 核验记录

- 本人实读：`fkv_task_write/src/lib.rs:1055-1079/1297-1402`（grep rebalance/
  quarter 定位）、`00-index.md:99`。
- 子代理摘录（未复核）：C++ 行号。

## 审查结论（dragon 复核，2026-11-04）

**成立（功能缺口 + 追踪断链都属实），留 todo 给 `fkv_task_write` owner。**

- 数据页有 rebalance（`fkv_task_write/src/lib.rs:1055-1079`，quarter/three_quarter/
  restart_num>1 三条件齐备），索引页构建（`build_index_pages` / `build_index_page_li`）
  只有「放不下就 finish 新页」—— 实读确认无 quarter 重分配分支。
- C++ 侧确认（`eloqstore @ 82b774c`，`src/tasks/batch_write_task.cpp:990-1003`
  `FinishIndexPage`）：`cur_page_len < one_quarter && prev_handle &&
  prev_handle->RestartNum() > 1 && prev_handle->ContentLength() > three_quarter`
  → `Redistribute(prev_handle, page_view, cur_page_key)`（索引版 `:1189-1265`）。
- **追踪断链属实**：`issue/new/00-index.md` 记 `core-15` 已 reject（判为与 eng-53
  重复），eng-53 的结论把该项 defer 回 core-15 后关闭 —— 两边互踢，当前**无 open
  issue** 承接。本条（sto-67）即该缺口的唯一归口，不得再 defer 回去。

## 阻塞（留 todo）

`fkv_task_write/src/lib.rs` 属另一会话热点（本轮多次秒级修改）。修法要点（供 owner）：

1. 索引页构建加 quarter 重分配：`builder.finish()` 之前，若
   `prev` 页 restart_num > 1 且 content_len > three_quarter 且「新页将只放
   `< 1/4` 的内容」，走 Redistribute（把 prev 尾部的 entry 搬到新页头）。
2. 与数据页 rebalance 共用「三分之一/四分之一」判定口径，避免两套阈值各自漂移。
3. 测试：构造大索引树，断言**不存在** content_len < 1/4 的尾页（反向验证：
   关掉重分配分支该断言必须失败），并断言树高 / 页数与 C++ oracle 差分口径一致
   （01 oracle 的 index 页类语料已就位）。

## DONE（2026-11-04）

- `fkv_task_write/src/lib.rs` 新增 `rebalance_index_pages`：`build_index_page_li` 产出分页后，
  尾页 `content_len < page_size/4`、前页 `> 3/4` 且前页 `restart_num > 1` 时，按 C++
  `BatchWriteTask::Redistribute` 索引分支把前页尾部 **restart region** 迁到尾页头部：
  - 保留区段自 `INDEX_HEADER_SIZE + 2` 起逐 region 累加到 `(prev_len + cur_len)/2`
    字节中点（对齐 `average_len`），前页用 `FastIndexPageBuilder` 直拷保留 region；
  - 新尾页以首个迁移 entry 作 leftmost（键丢弃）、该键改为页分隔键，随后接原尾页
    leftmost 指针（键用旧分隔键）及其余 entry，按 `index_restart_interval` 重新分块；
  - 全保留（无法迁移）时返回 `false` 不动页，避免破坏盘面。
- 抽出 `rebalance_bounds` / `needs_quarter_rebalance`，数据页与索引页共用
  quarter / three_quarter / restart_num>1 判定，阈值不再两套漂移。
- 测试（`fkv_task_write` 单元测试）：
  - `index_page_quarter_redistribution_balances_tail`：先按无重分配路径构造
    prev>3/4、cur<1/4 的稀疏尾页（断言三条件成立），重分配后两页均落在
    [1/4, 3/4]，children 顺序与页分隔键正确；
  - `build_index_page_li_rebalances_sparse_tail`：端到端验证尾页不再 < 1/4、
    children 顺序不变。
- 验证：`cargo check -p fkv_page -p fkv_btree -p fkv_task_write` 通过；
  `cargo nextest run -p fkv_page -p fkv_btree -p fkv_task_write -p fkv_store
  --no-fail-fast` 198 passed；三 crate clippy `--no-deps --all-features
  --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

结论：**通过**（断链已由本条承接并闭环）。

- `fkv_task_write/src/lib.rs:1570-1645` `rebalance_index_pages`：`needs_quarter_rebalance` 三条件（:1581）、`(prev_len+cur_len)>>1` 字节中点（:1586）、`PageRegionIter` region 直拷前页（:1634-1640）、迁移首 entry 作新 leftmost + 旧分隔键接原尾页（:1612-1631）、全保留返回 false 不动盘面（:1598-1601）。
- `build_index_page_li` 末尾接线（:1679）；`:1133-1143` `rebalance_bounds`/`needs_quarter_rebalance` 数据/索引两用，阈值单点定义。
- 测试 `:2208` `index_page_quarter_redistribution_balances_tail`（先复刻稀疏尾页断言三条件 :2233-2235，重分配后两页落 [1/4,3/4] :2244-2250、children 顺序不变 :2252-2253、尾页首键已变 :2255）、`:2260` 端到端。断言真实覆盖。
