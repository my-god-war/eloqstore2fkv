# core-22b 云端读对象 key 按 manifest 的 per-file (branch, term) 解析

## 位置
- `crate/io/fkv_io/src/lib.rs`：`PageIo::set_file_range_li` 默认钩子。
- `crate/engine/fkv_store/src/lib.rs`：`install_replay_state` 安装时同步 `branch_metadata.file_ranges`。
- `crate/engine/fkv_cloud/src/lib.rs`：`file_range_map`、`range_ident`、`ranged_name`、读/写 key 分离。

## 方案
- 新增 `PageIo::set_file_range_li(&self, tbl, &[BranchFileRange])`（默认 no-op），对齐 C++ `SetBranchFileMapping`。
- CloudStore 按表缓存链；`partition_point` 分别按 `max_file_id` / `max_segment_file_id` 二分（两字段跨区间单调不减）。
- 读 / 删 key 走链解析；链为空或 file id 超出链尾（当前 term 新分配）回退当前 identity。
- 写路径（`ensure_resident` / `flush_tbl` / `flush_segment_tbl`）固定用当前 identity，绝不回退旧 term key。
- `evict_table` 同步清理链，避免残留旧 manifest 视图。

## 验收
- 旧 term / 父分支的 data / segment 对象按写时 key 读取；新分配 file id 用当前 identity。
- 测试：`crate/engine/fkv_cloud/tests/term_key.rs`。

## DONE（2026-11-04）
- 实现：`PageIo` 默认钩子 + store 安装时同步 + cloud 链缓存与读/写 key 分离；删除路径同样按链解析，evict 时清链。
- 测试：`crate/engine/fkv_cloud/tests/term_key.rs`（旧 term key 命中、链尾回退 identity、Store open_branch 后读父分支数据端到端）。
- 验证：`cargo nextest run -p fkv_store -p fkv_cloud` 96 passed；clippy 零警告（含 `fkv_io`）。
- 遗留：云端 manifest 跨 term 选择/提升（C++ `GetManifest` 的 promote）未实现，跨 term 云 reopen 仍会先读不到新 term manifest，另行立案。
- 遗留 2：`remote_file_id_page` 仍只列当前 identity 的文件，旧 term 对象不在 GC 候选集内；配合 `list_file_id_li` 的远端列举缺口另行处理。

## 复审（2026-11-05 r6b）

通过。

- `PageIo::set_file_range_li` 默认 no-op 钩子：`crate/io/fkv_io/src/lib.rs:19`；CloudStore 实现：`crate/engine/fkv_cloud/src/lib.rs:1745`；store 安装时同步：`crate/engine/fkv_store/src/lib.rs:1792`（`set_file_range_li(tbl, &replay.branch_metadata.file_range_li)`）。
- 测试断言覆盖修复点：`crate/engine/fkv_cloud/tests/term_key.rs:71`（per-file branch/term key 命中）、`:116`（open_branch 读父分支数据端到端）、`:163`（最新 term manifest 选择）、`:203`（跨 term reopen 读旧 term 数据）、`:274`（远端列举含旧 term，GC 候选补遗）。
- DONE 遗留 1（跨 term manifest promote）已由 `cld-56`（reviewed/）承接；遗留 2 的列举面在 term_key.rs:274 已部分覆盖，无悬空声称。
