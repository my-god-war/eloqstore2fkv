# eng-74a manifest 追加 / 原子写收敛为私有 helper

## 位置

`crate/io/fkv_io_compio/src/lib.rs`

- `append_manifest` / `append_manifest_file`
- `switch_manifest` / `write_manifest_file`
- `tmp_manifest_path`

## 方案

1. 新增 `async fn append_file_at(&self, dir: &Path, path: &Path, record: &[u8]) -> Result<()>`：
   承接 `open_write_file` → 偏移缓存 / `metadata` 首查 → failpoint `AppendManifest:Write` →
   `take_write_buf` → `write_at` → 更新偏移 → `sync_data` → `sync_manifest_dir_once`。
2. 新增 `async fn atomic_write_file(&self, dir: &Path, path: &Path, record: &[u8]) -> Result<()>`：
   `File::create(tmp)` → `write_whole` → `sync_data` → failpoint `AtomicWriteFile:Sync` / `Rename`
   → `rename` → `forget_fd` → `sync_dir`。
3. 新增自由函数 `fn tmp_path(path: &Path) -> PathBuf`（追加 `.tmp`），删除 `tmp_manifest_path`。
4. 四个 `ManifestIo` 方法改为「`ensure_dir` + 选路径 + 调 helper」。

## 验收

- 四个方法的 failpoint 名称、顺序、fsync 次序与合并前逐行一致。
- `cargo check -p fkv_io_compio` 通过；`tests/failpoint.rs` 通过。

## DONE（2026-11-04）

实现要点：

- `append_file_at(dir, path, record)`：偏移缓存 → 可选 fstat → failpoint `AppendManifest:Write` → 写池缓冲 `write_at` → 更新偏移 → `AppendManifest:Sync` → `sync_manifest_dir_once(dir)`。
- `atomic_write_file(dir, path, record)`：`File::create(tmp)` → `write_whole` → `sync_data` → `AtomicWriteFile:Sync` / `Rename` → `rename` → `forget_fd` → `sync_dir(dir)`。
- `tmp_path(path)` 统一 tmp 名（追加 `.tmp`），删除 `tmp_manifest_path`。
- `append_manifest` / `append_manifest_file` / `switch_manifest` / `write_manifest_file` 各自只剩 `ensure_dir` + 路径选择 + 一次 helper 调用。

failpoint 名称、kill/fail 顺序、fsync 与 rename 次序与合并前逐行一致。

验证：

- `cargo check -p fkv_io_compio` 通过。
- `cargo nextest run -p fkv_io_compio`：29 passed（含 `failpoint::io_failpoints_inject_errors`、`compio::manifest_roundtrip`）。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:1632 append_file_at`（偏移缓存→可选 fstat→
  `AppendManifest:Write` fail/kill→写池 write_at→更新偏移→Sync→
  sync_manifest_dir_once）、`:1663 atomic_write_file`（create tmp→write_whole→
  sync→`AtomicWriteFile:Sync`/`Rename` fail/kill→rename→forget_fd→sync_dir）、
  `:706 tmp_path`；`tmp_manifest_path` 已无。
- failpoint 名称与顺序抽读属实（`:1640/1654/1668-1676`）。
