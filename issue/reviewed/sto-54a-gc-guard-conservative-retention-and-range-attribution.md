# sto-54a GC in-flight guard：缺边界保守保留 + (branch, term) 区间归属

## 位置
- `crate/engine/fkv_task_gc/src/lib.rs`：`InflightGuard::{add_ranges, fold, seed, seed_segment, retain, entry_inflight, active_bound}`、`sort_chain`
- `crate/engine/fkv_task_gc/tests/inflight.rs`
- `crate/engine/fkv_store/tests/gc_inflight.rs`

## 背景
core-23 已落地 per-branch `BranchGuard`（`least_unflushed`）与 `Store::gc` 的 in-memory 播种；
sto-55 已落地旧 manifest 剪枝与空目录收尾。剩下 sto-54 的两块：

1. `file_ranges` 归属解析是线性扫描 + 首个最大上界胜出，结果依赖 manifest 读取顺序，
   且 `(branch, term)` 键没有落到代码结构里；
2. C++ 对「缺 BranchGuard」的 branch 保守保留文件，Rust 无此分支：区间上界为 0
   （branch 刚创建 / manifest 落后）时守卫被当成「无保护」，可删掉对方未 flush 的首批文件；
   另外本进程没有 active branch（表未打开、别的实例在写）时，高水位之上的文件同样无保护。

## 方案
1. `InflightGuard` 内部改为「branch 名 → 下标」+ 平行 `guard_li`；
   `data_li` / `segment_li` 保存 `RangeEntry { max, branch_idx, term }`，按上界升序排序并合并
   同 `(branch, max)`（保留最新 term），归属用 `partition_point` 二分，O(log R)。
2. 归属取「上界不超过 file id 的最近区间」；同一上界可能有多个 branch
   （父 branch 预留下界 == 子 branch 起始），任一 branch 判定 in-flight 即保留，消除顺序依赖。
3. 保守保留：
   - 归属 branch 无正高水位（边界 None）且整条链存在正高水位 → 保留；
   - file id 高于全部上界且本进程无 active branch → 保留；
   - 整条链全为 0（truncate 后 / 空表）→ 0 不携带信息，维持原语义，保证 truncate 后可回收。

## 验收
- `fkv_task_gc::inflight`：
  - `guard_resolves_tied_high_water_order_independently`：同上界两种折入顺序结果一致；
  - `guard_conservative_when_bound_or_active_missing`：无边界 / 无 active 时保留；空守卫不保留。
- `fkv_store::gc_inflight::gc_without_open_table_keeps_files_above_manifest_high_water`：
  未打开表的 GC 不删高水位之上的文件。
- `cargo nextest run -p fkv_task_gc -p fkv_gc -p fkv_store -p fkv_runtime --no-fail-fast` 全绿。
- clippy 三 crate `--no-deps --all-features --all-targets -- -D warnings` 零警告。

## 不做（记录理由）
- retained 集合升级为 `HashSet<RetainedFileKey>`：C++ 会丢弃「不在本 branch 区间」的引用，
  而子 branch manifest 继承父 mapping，父 manifest 被 sto-55 剪枝后该引用是唯一来源；
  丢弃会引入误删。Rust 保持并集（保守超集），(branch, term) 维度只用于 in-flight 归属判定。

## DONE（2026-11-04）

- `crate/engine/fkv_task_gc/src/lib.rs`
  - `InflightGuard` 内部重构：`branch_li` / `branch_map` / 平行 `guard_li`，`active` 改存下标；
    `data_li` / `segment_li` 存 `RangeEntry { max, branch_idx, term }`（对齐 manifest 的
    `(branch, term)` 键），`sort_chain` 按上界升序排序并合并同 `(branch, max)` 保留最新 term。
  - `retain` 用 `partition_point` 二分定位「上界不超过 file id 的最近区间」，同上界多 branch
    取「任一 in-flight 即保留」，消除 manifest 读取顺序依赖。
  - 保守保留：归属 branch 无正高水位且链上存在正高水位 → 保留；高于全部上界且无 active
    branch → 保留；整链全 0（truncate 后 / 空表）保持原语义（0 不携带信息），truncate 仍可回收。
- `crate/engine/fkv_task_gc/tests/inflight.rs`：新增 2 个用例（同上界顺序无关、缺边界/缺 active 保守保留）。
- `crate/engine/fkv_store/tests/gc_inflight.rs`：新增未打开表 GC 的保护用例。

验证：

- `cargo nextest run -p fkv_task_gc -p fkv_gc -p fkv_store -p fkv_runtime --no-fail-fast` → 138 passed, 1 skipped。
- `cargo clippy -p fkv_task_gc -p fkv_gc -p fkv_store --no-deps --all-features --all-targets -- -D warnings` → 零警告。

遗留：

- retained 集合仍为 `HashSet<FilePageId>` 并集（理由见「不做」）；云端跨实例场景只有
  「高水位之上保留」这一层保护，跨实例 term 竞争测试仍未覆盖。

## 复审（2026-11-05 r6）

通过。`fkv_task_gc/src/lib.rs:64-79`（`InflightGuard`：`branch_li`/`branch_map`/平行
`guard_li`/`data_li`/`segment_li` 存 `RangeEntry{max, branch_idx, term}`）、
`:270-280`（`sort_chain` 按上界排序并合并同 (branch,max) 保留最新 term）、
`:211`（`partition_point` 二分归属）、`:218-243`（同上界多 branch 任一 in-flight 即保留；
owner 上界 0 交 active 兜底；高于全部上界且无 active 时「链上存在正高水位才保守保留」）。
测试 `fkv_task_gc/tests/inflight.rs:63`（同上界顺序无关）、`:119`（缺边界/缺 active 保守
保留 + 空守卫不保留）、`fkv_store/tests/gc_inflight.rs:48`（未打开表 GC 不删高水位之上
文件）均存在。
