# sto-51a branch manifest 损坏尾部从不覆盖

## 位置
- `crate/engine/fkv_store/src/lib.rs:1333`（`open_branch`）
- `crate/engine/fkv_store/src/lib.rs:1447`（`reopen` 的 branch 分支）
- `crate/io/fkv_io/src/lib.rs:85`（`ManifestIo`）
- `crate/io/fkv_io_compio/src/lib.rs:2031`（`reset_manifest`）
- `crate/io/fkv_io_mem/src/lib.rs:131`（`MemStore` manifest）

## 现状
core-20 只修复 main 活跃 manifest：`Store::reopen` 用 `reopen_with_size` 返回的 valid_len
调用 `reset_manifest` 截断（`crate/engine/fkv_store/src/lib.rs:1458`）。

branch 路径未覆盖：
- `open_branch` 用 `replay_manifest_bytes` + `bytes.len()`，valid_len 丢失；
- 随后 `if self.manifest_file.is_none()` 判断，而此时 `manifest_file` 刚被置为 `Some`，
  截断永不执行；
- `Store::reopen`（branch 模式）同样用 `bytes.len()`。

后果与 sto-51 相同：branch manifest 尾部 torn / 损坏 → open_branch 成功 →
下一次写追加到垃圾之后 → 再 open_branch 读到「损坏 + 完好」返回 `Corrupted`，
或垃圾 header 的 payload_len 超大导致后续 record 被静默忽略（丢已确认写）。

## 方案
1. `fkv_task_reopen` 增加 `replay_manifest_bytes_with_len`，一次切分同时返回
   `ReplayState` 与 valid_len（等价 C++ `file_size_before_corrupted_log_`）。
2. `ManifestIo` 增加 `reset_manifest_file(tbl, name, size)`，默认 no-op。
3. `CompioStore` 实现：写 `manifest_offset` 后 `set_len(size)`。
4. `MemStore` 实现 `reset_manifest` / `reset_manifest_file`：对内存镜像 `truncate`。
5. `MeteredIo` 透明转发。
6. `open_branch` / branch 模式 `reopen` 改用 `replay_manifest_bytes_with_len`，
   install valid_len 并无条件调用 `reset_manifest_file`。

## 验收
- 新增 `compio_branch_reopen_truncates_corrupted_tail`：branch manifest 尾部注入垃圾 →
  open_branch → 再次写 → 再 open_branch，断言不报错且两次写都可见，文件长度回退到 valid_len。
- 新增 `compio_reopen_truncates_corrupted_tail`：main 路径同场景回归。
- `cargo nextest run -p fkv_task_reopen -p fkv_store --no-fail-fast` 全绿；
  clippy `-D warnings` 零警告。

## DONE（2026-11-04）
- 改动：
  - `crate/engine/fkv_task_reopen/src/lib.rs`：新增 `replay_manifest_bytes_with_len`。
  - `crate/io/fkv_io/src/lib.rs`：`ManifestIo::reset_manifest_file`（默认 no-op）。
  - `crate/io/fkv_io_compio/src/lib.rs`：`reset_manifest_file` 截断具名 manifest。
  - `crate/io/fkv_io_mem/src/lib.rs`：`reset_manifest` / `reset_manifest_file` 截断内存镜像。
  - `crate/foundation/fkv_metrics/src/lib.rs`：转发 `reset_manifest_file`。
  - `crate/engine/fkv_store/src/lib.rs`：`open_branch` / branch `reopen` 用 valid_len 截断。
  - 测试：`crate/engine/fkv_store/tests/compio_manifest.rs`、`.../branch_store.rs`。
- 验证：95 tests passed；clippy 零警告。

## 残留（范围外）
- `CloudStore` 与 `StoreBackend` 未覆写 `reset_manifest_file`，走默认 no-op，行为与本次改动前一致。
  远端对象 manifest 的尾部截断需要对象存储侧支持（或 cloud 转发到本地缓存），留待 cloud 专项。

## 复审（2026-11-05 r6）

通过。`fkv_task_reopen/src/lib.rs:110-121`（`replay_manifest_bytes` /
`replay_manifest_bytes_with_len`，后者返回 valid_len 并注明等价 C++
`file_size_before_corrupted_log_`）；`fkv_io/src/lib.rs:108`（`ManifestIo::reset_manifest_file`
默认 no-op）；`fkv_store/src/lib.rs:1668`（`open_branch` 按 valid_len 调
`reset_manifest_file`）。测试 `fkv_store/tests/branch_store.rs:180`
（`compio_branch_reopen_truncates_corrupted_tail`）存在。残留（CloudStore/StoreBackend
走默认 no-op）已在 DONE 记录，归 cloud 专项。
