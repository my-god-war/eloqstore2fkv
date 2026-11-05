# sto-55a 本地旧 term manifest 剪枝与空分区目录收尾

## 位置
- `crate/io/fkv_io/src/lib.rs`：`ManifestIo::prune_superseded_manifest_files`（默认实现）、`FileIo::try_remove_empty_partition_dir`（默认 no-op）。
- `crate/io/fkv_io_compio/src/lib.rs`：`impl FileIo for CompioStore::try_remove_empty_partition_dir`。
- `crate/engine/fkv_cloud/src/lib.rs`：`CloudStore` 覆盖 `prune_superseded_manifest_files` 为 no-op。
- `crate/engine/fkv_store/src/lib.rs`：`Store::gc` 尾部调用剪枝 + 目录收尾。

## 方案
1. 剪枝放在 data/segment 删除之后，不影响本轮 retained set。
2. 只删同 active branch、无 tag、`term < 当前 term` 的 manifest；必须已存在当前 term manifest（`manifest_<branch>_<term>`）才判定旧文件被取代。`main`（活跃文件名固定 `manifest_main_0`）与 `term == 0` 直接跳过。
3. 空目录收尾：目录内任何条目（data / segment / manifest / `.tmp` / 未知残留）存在就保留，只 `remove_dir`，绝不递归删除；删除后清理 `dir_ready` / `dir_index_map` / fd 缓存 / `dirty_dir`。
4. 云端显式关闭：`CloudStore::remove_manifest_file` 会删远端对象，远端剪枝见 sto-55b。

## 验收
- 新增 `crate/engine/fkv_store/tests/gc_cleanup.rs`：
  - 只删被当前 term 取代的 active branch 旧 manifest；当前 term、其它 branch、archive、main 保留；
  - 无当前 term manifest 时一个都不删；
  - `drop_table` 后的空目录被 GC 移除；
  - truncate 后 manifest 仍在、目录含未知残留文件时，目录保留（不误删）。
- `cargo nextest run -p fkv_store --test gc_cleanup` 4 passed。
- `cargo clippy -p fkv_store -p fkv_task_gc --no-deps --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

- `crate/io/fkv_io/src/lib.rs`
  - 新增 `ManifestIo::prune_superseded_manifest_files` 默认实现（同 branch / 无 tag / 旧 term / 必须有当前 term manifest）。
  - 新增 `FileIo::try_remove_empty_partition_dir`，默认 no-op。
- `crate/io/fkv_io_compio/src/lib.rs`
  - `CompioStore` 实现空目录移除：目录非空（含未知残留）一律保留，`remove_dir` 后失效目录索引 / fd / 脏标记缓存。
- `crate/engine/fkv_cloud/src/lib.rs`
  - `CloudStore` 覆盖旧 manifest 剪枝为 no-op，避免本轮误删远端对象。
- `crate/engine/fkv_store/src/lib.rs`
  - `Store::gc` 删除 data/segment 后剪枝旧 term manifest，再尝试移除空分区目录。
- `crate/engine/fkv_store/tests/gc_cleanup.rs`（新增 4 个用例）

验证：

- `cargo nextest run -p fkv_task_gc -p fkv_store -p fkv_runtime --no-fail-fast` → 130 passed，1 skipped。
- `cargo nextest run -p fkv_store --test gc_cleanup` → 4 passed。
- `cargo clippy -p fkv_task_gc -p fkv_store -p fkv_io -p fkv_io_compio -p fkv_cloud --no-deps --all-features --all-targets -- -D warnings` → 零警告。

遗留：

- `issue/todo/sto-55b-cloud-remote-superseded-manifest-prune.md`（远端旧 manifest 删除）。
- `issue/todo/sto-55c-truncate-empty-table-dir-cleanup.md`（truncate 后活跃 manifest 与目录收尾，需 Store 状态机联动）。

## 复审（2026-11-05 r6）

通过。`fkv_io/src/lib.rs:197`（`ManifestIo::prune_superseded_manifest_files` 默认实现）、
`:538`（`FileIo::try_remove_empty_partition_dir` 默认 no-op）；`fkv_store/src/lib.rs:3348`
（`Store::gc` 尾部先剪枝旧 term manifest）、`:3352`（再尝试移除空分区目录，顺序正确：
数据文件删除之后）。测试 `fkv_store/tests/gc_cleanup.rs`（4 个用例，实测计数）存在。
