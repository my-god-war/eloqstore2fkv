# sto-54 FileGc 缺 in-flight 写保护（BranchGuard）与 (branch,term) retained key

## 位置
- Rust: `crate/engine/fkv_task_gc/src/lib.rs:127-144`（`gc_files_with_retained`）、`96-117`（`manifest_retained_file_page_set`）、`crate/engine/fkv_gc/src/lib.rs:7-15`（`retained_file_page_set` 只有 `FilePageId`，无 branch/term维度）、`crate/engine/fkv_store/src/lib.rs:2222-2243`（`Store::gc`：并集后直接删除不在 retained 的文件）
- C++: `src/file_gc.cpp:1191-1214`（`DeleteUnreferencedLocalFiles` 的 BranchGuard 判定）、`src/file_gc.cpp:33-93`（`GetRetainedFiles` 按 `RetainedFileKey{file_id, branch, term}` 归集 + `GetBranchNameAndTerm` 区间解析）、`include/file_gc.h:36-56`（`BranchGuard` 结构体与语义注释）

## 现状
Rust retained set = 内存 mapping（`add_mapping_retained`，`fkv_task_gc:87-93`）∪ 所有 manifest（active + branch + archive，D55）的 mapping，元素为裸 `FilePageId`（`fkv_gc:7-15`），删除判定只看 `file_id 是否在 retained_file_set`（`gc_files_with_retained:133-142`）。

缺少的语义：
1. **in-flight 写保护**：C++ 为每个 branch 维护 `BranchGuard{newest_term, least_unflushed_file_id, least_unflushed_seg_file_id}`，对 `term >= newest_term && file_id >= least_unflushed_file_id` 的文件一律保留（`file_gc.cpp:1207-1214`）——包括「当前正在写、manifest 尚未 flush 的边界文件」，以及 manifest 落后于并发 primary 新 term 的场景（`file_gc.h:75-83` 注释）；
2. **branch/term 维度**：C++ retained key 带 branch/term，`GetRetainedFiles` 用 `GetBranchNameAndTerm(file_ranges, ...)` 解析每个引用文件属于哪个 branch 区间（`file_gc.cpp:54-64`），不在区间内的引用**不保留**并告警；Rust 一律保留（方向安全，但判定集合不同源）；
3. **缺 guard 的保守保留**：C++ 对非 active branch 缺 BranchGuard 时保守保留文件（`file_gc.cpp:1197-1205`）；Rust 无此概念。

## C++ 实现
`ExecuteLocalGC`（`file_gc.cpp:239-375`）先 `SeedActiveBranchGuardFromInMemory`（从 RootMeta allocator `MaxFilePageId()` 播种 active branch guard，118-165 行），再 `AugmentRetainedFilesFromBranchManifests`（660-811 行，从磁盘 manifest 的 `BranchFileRange` 折叠 `std::max` 合成 guard），删除时两个条件叠加：`!retained_files.contains(key)` **且** 不被 in-flight guard 保护（1183-1214 行）。

## 差异与风险
- 当前 Rust 单进程单 shard、GC 与写同线程串行，in-flight 场景靠内存 mapping 并集兜住，本地模式实际安全；但：
  - **云/多实例**：Rust CloudStore GC（`fkv_cloud`）与其他实例并发写时，本实例 manifest 落后于新 term writer 的边界文件无保护，理论上可删除他人 in-flight 文件——todo.md 宣称 Cloud 模式完成，此保护缺失未验证（D32/D33 的测试覆盖不到跨实例 term 竞争）；
  - retained 判定不一致导致 GC 行为不可逐条对账（C++ 会丢弃无区间归属的引用文件，Rust 永远保留 → 垃圾文件回收率不同）；
  - 与 sto-51/52 联动：reopen 的 `max_fp_id`/分配器重建若也缺「逻辑尺寸」，边界判定缺数据源。

## 对齐建议
- `fkv_gc::retained_file_page_set` 升级为 `HashSet<RetainedFileKey>`（含 branch/term），由 `BranchManifestMetadata.file_ranges` + `GetBranchNameAndTerm` 等价逻辑解析归属；
- `manifest_retained_file_page_set` 增加 `BranchGuardMap` 输出：从各 manifest 的 `file_ranges` 按「同 branch 取最大 term、同 term 取 max file_id/segment_file_id」折叠（对齐 `ProcessOneManifest` 617-651 行），active branch 再以内存 `max_fp_id >> shift` 播种；
- `gc_files_with_retained`/`gc_segment_files_with_retained` 增加 guard 检查：`term >= guard.newest_term && file_id >= guard.least_unflushed` → 保留；
- `Store::gc` 把 branch/term 感知的 retained+guard 传入执行层；内存 mapping 兜底逻辑保留。

## 性能优化建议
- guard 检查是每文件 O(1) hash lookup，无额外 IO；retained key 升级后保留集更小，删除判定更快。

## 严重度
P1（行为不一致/功能缺失）

## 审查结论（2026-11-04）

逐条核验：核心语义已由 core-23 + sto-55 覆盖，剩余部分成立并已实现。

已覆盖：

- **in-flight 写保护**：`InflightGuard` / `BranchGuard`（`crate/engine/fkv_task_gc/src/lib.rs`）已从 `BranchManifestMetadata.file_ranges` 折出 per-branch `least_unflushed`，`Store::gc` 用 `mapper.max_fp_id() >> shift` 播种 active branch；`gc_files_with_retained` / `gc_segment_files_with_retained` 已做边界判定。对应 core-23。
- **retained 集合来源**：`fkv_gc::retained_file_page_set` 已在 eng-78 删除，现由 `add_mapping_retained` 统一填充；active + branch + archive manifest 全部计入，方向安全。
- **旧 manifest / 空目录收尾**：sto-55a 已落地，`Store::gc` 在删除后剪枝旧 term manifest 并只移除空目录。

本 issue 剩余成立的部分：

1. **缺 BranchGuard 保守保留**（C++ `file_gc.cpp:1197-1205`）：区间上界为 0（branch 刚创建、manifest 落后）时 Rust 把守卫当「无保护」→ 可删掉对方未 flush 的首批文件；本进程无 active branch（表未打开、别的实例在写）时，高水位之上的文件同样无覆盖。
2. **(branch, term) 归属**：原解析是「首个最大上界胜出」线性扫描，同类高水位命中哪个 branch 取决于 manifest 读取顺序。
3. **retained key 带 (branch, term)**：判定为不采纳。C++ 会丢弃不在本 branch 区间的引用，但子 branch manifest 继承父 mapping，sto-55 剪枝后该引用可能是唯一来源，丢弃即误删。Rust 保持并集（保守超集）；(branch, term) 维度落在 in-flight 归属链上。

> 更正（2026-11-04，dragon 实核 + `eng-88` 证据）：上面这句**不成立**。
> `RangeEntry.term` 只写不读 —— `sort_chain` 里保留同 `(branch, max)` 的最大 term，
> 但 `entry_inflight`（`fkv_task_gc/src/lib.rs:207-223`）只用 file id 比
> `guard_bound`，term 不参与任何判定。C++ 的保护条件是
> `term >= newest_term && file_id >= least_unflushed_file_id`
> （`file_gc.cpp:1207-1208`），term 维度真实参与。残留归 `eng-88`（todo）。

实现见 `issue/todo/sto-54a-gc-guard-conservative-retention-and-range-attribution.md`（已 DONE）。

## DONE（2026-11-04）

`sto-54a` 已完成：GC guard 保守保留 + 按区间归属 branch，(branch, term) retained key
判定为不采纳（并集是更安全的超集）。父 issue 归档。

## 复审（2026-11-05 r6）

通过（父条目）。in-flight 保护（core-23）与保守保留/区间归属（sto-54a）、term 维度
（eng-88）均已在当前源码复核，见各自 r6 复审小节；dragon 更正块保留在案。
