# eng-60 create_branch 不做 pre-compact，且空分区/未打开表也写 branch manifest

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:1613-1708`（`create_branch` 直接 `snapshot_record` + `write_manifest_file`，全程无 compact 调用）；`:1619`（`self.mapper_map.get(tbl).map_or(0, PageMapper::max_fp_id)` —— 表未打开/空表以 0 为基线照常写出 branch manifest）
- C++: `src/tasks/background_write.cpp:622-636`（注释明确："Compact before snapshotting so the branch inherits a dense mapping and does not carry over fragmented files from the parent."，`if (Option()->data_append_mode) { Compact(); ... }`）；`:646-662`（root `NotFound` 或 mapper 为空的 stub 分区 → "No branch manifest needed — treat as success." 直接返回，不创建 branch manifest）

## 现状
fkv 分支直接继承父的碎片化 mapping；空分区多出一个 branch manifest 文件。

## C++ 实现
pre-compact 让 branch 起点是稠密 mapping（后续该 branch 的文件上传/GC 效率）；空分区不落文件。

## 差异与风险
1. 分支文件布局碎片化 → 云上传对象数、GC retained 集合放大（云端整文件上传时更明显，关联 10a）；
2. 目录列举、GC、跨语言工具看到的文件集合与 C++ 不同（oracle 差分会体现）。

## 对齐建议
- append 模式下 `create_branch` 先 `compact`（复用 `compact_if_needed`/全量 compact 语义）；
- root 缺失或 mapper 为空时返回 `Ok` 且不落 branch manifest。

## 严重度
P1（布局与文件集合分歧）

## 核验记录（2026-11-04）
C++ pre-compact 注释与空分区分支实读确认（`background_write.cpp:620-664`）；Rust `create_branch` 无 compact、`map_or(0)` 基线实读确认。

## 审查结论（2026-11-04）

成立，且已修复。以实际源码核验：`create_branch` 无 compact；对未打开表通过
`entry(tbl).or_default()` 建 state 后照写 branch manifest。C++ 实读（GitHub 上游
`src/tasks/background_write.cpp`）确认：`data_append_mode` 先 `Compact()`，
`FindRoot` NotFound / `mapper_ == nullptr` 直接返回成功不落文件。

拆分与实现：

- `issue/done/eng-60d-create-branch-empty-partition-and-precompact.md`：空分区短路 + data 文件 pre-compact，已实现。
- `issue/done/eng-60b-segment-precompact-for-branch.md`：segment 文件 pre-compact，遗留（依赖 segment compaction 能力）。

## DONE（2026-11-04）

- 改动：`crate/engine/fkv_store/src/lib.rs`（`create_branch` 空分区短路 + `compact_before_branch`）、`crate/engine/fkv_store/tests/branch_store.rs`（2 个测试）。
- 验证：`cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast` 121 passed / 1 skipped；`cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings` 零警告。
- 未提交（禁止 git commit）。

## 复审（2026-11-05 r6b）

通过。

- 空分区短路：`crate/engine/fkv_store/src/lib.rs:2818-2827`——`root.is_some() && mapper.is_some()` 不满足即返回空 `BranchInfo { name: "", bytes: 0 }`，不落 branch manifest。
- pre-compact：`:2830` `create_branch` 调 `compact_before_branch`（`:2937`）。
- 测试断言覆盖修复点：`crate/engine/fkv_store/tests/branch_store.rs:305` `create_branch_skips_unopened_partition`（断言 manifest 文件列表为空 `:314-320`）；`:327` `create_branch_precompacts_fragmented_parent`（4 轮写后断言 branch 后文件数 < 写入时 `:356-360`）。
- segment pre-compact 遗留已由 `eng-60b`（reviewed/）承接；`eng-60d` 亦在 reviewed/。
