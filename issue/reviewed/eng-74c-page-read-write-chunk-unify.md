# eng-74c 页读写 direct / buffered 分支收敛

## 位置

`crate/io/fkv_io_compio/src/lib.rs`

- `read_page`（direct / buffered 两套重试 + 归还 + checksum）
- `write_page`、`write_page_li`（两套 `write_at` 提交）

## 方案

1. 新增 `enum ReadChunk { Plain(Vec<u8>), Direct(DirectBuf) }` 与
   `read_page_chunk(file, len, offset)` / `put_read_chunk(len, chunk)`：
   按 `direct_io` 选缓冲与 `read_at`，只在 helper 内 match 一次。
2. `read_page` 的重试匹配、短读判断、`dst` 拷贝、checksum 校验只写一份；
   候选路径循环与 `KvError::Corrupted` 收尾保持不变。
3. 复用已有 `WriteChunk`：新增 `write_chunk(file, chunk, offset) -> io::Result<usize>`，
   direct 写后归还对齐池，plain 写后归还写池；`write_page` / `write_page_li` 共用。

## 验收

- 短读同路径最多重试 3 次、`TryAgain` 透传、checksum 失败换下一候选路径、短写 `KvError::IoFail`
  与合并前完全一致。
- `cargo nextest run -p fkv_io_compio`、`cargo clippy -p fkv_io_compio --all-features --all-targets -- -D warnings` 全绿。

## DONE（2026-11-04）

实现要点：

- 新增 `ReadChunk { Plain(Vec<u8>), Direct(DirectBuf) }`（`as_slice` 统一视图）、
  `read_page_chunk`（按 `direct_io` 选缓冲并发起一次 `read_at`）、`put_read_chunk`（归还对应池）。
- `read_page` 的短读判断、`TryAgain` 重试（同路径最多 3 次）、`dst` 拷贝、checksum 校验只保留一份；
  候选路径循环与结尾 `KvError::Corrupted` 不变。
- 新增 `write_chunk(&mut File, WriteChunk, offset)`：direct 写后回对齐池，plain 写后回写池；
  `write_page` 与 `write_page_li` 共用。`write_page_li` 的 plain 组缓冲改为归还写池（仅分配策略变化）。

语义保持：`TryAgain` 透传、checksum 失败 `break` 后换下一候选路径、短读/短写仍分别重试 / `IoFail`。

验证：

- `cargo check -p fkv_io_compio` 通过。
- `cargo nextest run -p fkv_io_compio`：29 passed（含 `hardening::page_buffers_are_reused_across_io`、
  `hardening::read_page_reuses_read_buffer_and_keeps_dst_tail`、`batch::contiguous_pages_share_one_write_at`）。
- `cargo clippy -p fkv_io_compio --all-features --all-targets -- -D warnings` 零警告（无 `allow`）。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:1312-1324 read_page_chunk`（按 direct_io 选
  Plain/Direct 缓冲，`ReadChunk::as_slice` 统一视图）、`put_read_chunk`
  （read_page 内 `:1952/1964/1970/1974` 归还对应池）、`:1464-1483 write_chunk`
  （Direct 回对齐池 / Plain 回写池），`read_page` 重试/短读/checksum 单份实现
  （`:1940-1987`），`write_page`/`write_page_li` 共用。
