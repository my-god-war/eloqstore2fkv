# eng-60 create_branch 缺失 pre-compact，空分区仍写 branch manifest

## 审查结论（2026-11-04）

成立。C++ `BackgroundWrite::CreateBranch`（`src/tasks/background_write.cpp`）：

- `data_append_mode` 时先 `Compact()`；`Compact()` 返回 `NotFound`（分区无 manifest）→ 直接成功，不落文件。
- 随后 `FindRoot` 返回 `NotFound` 或 `meta->mapper_ == nullptr` → 「No branch manifest needed」直接成功。

Rust 原实现无 compact，且对未打开表 `entry(tbl).or_default()` 后照写 branch manifest。已按 fkv_store 最小范围修复。

## 已实现

位置：`crate/engine/fkv_store/src/lib.rs`

- `create_branch` 移入 `impl<I: PageIo + ManifestIo + FileIo>`；表未打开（`root == None`）或 mapper 缺失 → 返回 `BranchInfo { name: "", bytes: 0 }`，不创建 manifest / 不分配预留区间。
- 新增私有 `compact_before_branch`：按 `file_amplify_factor` 遍历非尾文件，放大率超阈值调用 `compact_file` 把 live page 搬到文件尾，随后 `gc` 删除搬空的旧文件。branch 因此继承稠密 mapping。
- `file_amplify_factor == 0` 沿用 Rust 既有语义「关闭自动 compact」，不触发 pre-compact（C++ 0 表示全量 compact，属既有配置语义差异，不在本 issue 改）。
- 测试：`crate/engine/fkv_store/tests/branch_store.rs`
  - `create_branch_skips_unopened_partition`
  - `create_branch_precompacts_fragmented_parent`

## 验收

- 空表 `create_branch` 返回 Ok 且 `list_manifest_file_name_li` 为空。
- 碎片父表 `create_branch` 后磁盘文件数下降，且不存在无 live page 的文件；branch 数据完整可读。
- `cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast` 全绿。
- `cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 遗留

- segment 文件 pre-compact 未实现，见 `eng-60b-segment-precompact-for-branch.md`。
- `BranchInfo` 在空分区返回空 name / 0 bytes：上层 `BranchReply` 原样透传，C++ 同样只返回成功。

## DONE（2026-11-04）

- 改动：`crate/engine/fkv_store/src/lib.rs`（`create_branch` 空分区短路 + `compact_before_branch`）、`crate/engine/fkv_store/tests/branch_store.rs`（2 个测试）。
- 验证：`cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast` 121 passed / 1 skipped；`cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings` 零警告。
- 未提交（禁止 git commit）。

## 复审（2026-11-05 r6）

结论：**通过**。

- 空分区短路：`fkv_store/src/lib.rs:2789` `create_branch`，`:2795-2805` 未打开表（root/mapper 缺失）直接返回 `BranchInfo { name: "", bytes: 0 }`，不建 manifest。
- pre-compact：`:2806` 调 `compact_before_branch`（data 臂 `:2919-2938` 按 `file_amplify_factor` 选文件搬页 + `:2947` gc）。
- 测试：`fkv_store/tests/branch_store.rs:280-294` 空分区断言 `list_manifest_file_name_li` 为空；`:299-350` `create_branch_precompacts_fragmented_parent` 断言文件数下降（:329-333）、无死文件（:334-337）、branch 数据完整（:341-348）。断言真实覆盖。
