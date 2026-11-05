# eng-74b read_file / file_size 与 data/segment 列举删除收敛

## 位置

`crate/io/fkv_io_compio/src/lib.rs`

- `read_file` / `file_size`
- `list_file_id_li` / `list_segment_file_id_li`
- `remove_file` / `remove_segment_file`
- `index_path` / `current_path` / `write_path` / `variant_path_li` / `forget_path` 的 `segment: bool`

## 方案

1. 新增 `enum FileKind { Data, Segment }`（编译期枚举，无 dyn），为 `DirIndex` 增加
   `primary` / `primary_mut` / `other` / `other_mut` 访问器，把 `segment: bool` 分支收敛到枚举 match。
2. 新增 `async fn open_data_file(&self, tbl, file_id) -> Result<Option<File>>`：
   `cached_data_path` → `open_read_file` → `NotFound` 时 `forget_path` + 重查一次。
   `read_file` → `read_whole`，`file_size` → `metadata().len()`。
3. 新增 `async fn list_file_id_by_kind(&self, tbl, kind)` 与
   `async fn remove_file_by_kind(&self, tbl, file_id, kind)`，四个公开方法一行转发。

## 验收

- 原有 error 语义不变：`NotFound` 返回 `None` / 忽略，其它 io 错误 `KvError::IoFail`。
- `cargo check -p fkv_io_compio`、`tests/naming.rs` 通过。

## DONE（2026-11-04）

实现要点：

- 新增 `enum FileKind { Data, Segment }`，`DirIndex` 提供 `primary` / `primary_mut` / `other` / `other_mut`；
  `index_path` / `current_path` / `write_path` / `variant_path_li` / `forget_path` 的 `bool segment` 全部改为 `FileKind`。
- `open_data_file`：`cached_data_path` → `open_read_file` → `NotFound` 时 `forget_path` + 重查一次；
  `read_file` 调 `read_whole`，`file_size` 调 `metadata`。
- `list_file_id_by_kind` / `remove_file_by_kind` 承载 `list_*` / `remove_*` 公共体，四个公开方法一行转发。

error 语义不变：首次与重查后的 `NotFound` 分别返回 `None` / 忽略，其它 io 错误 `KvError::IoFail`。

验证：

- `cargo nextest run -p fkv_io_compio`：29 passed（含 `naming::remove_file_clears_all_identity_variants`、`naming::stale_cached_path_is_recovered`、`hardening::truncated_page_and_segment_read_fail`）。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:128 enum FileKind`、`DirIndex::primary/primary_mut/
  other/other_mut`（`:135-157`）；`open_data_file:1707`（NotFound → forget_path
  重查一次）、`list_file_id_by_kind:1730`、`remove_file_by_kind:1752`，
  公开方法一行转发。
- `read_file` → `read_whole`（`:2569-2571` 已见）。
