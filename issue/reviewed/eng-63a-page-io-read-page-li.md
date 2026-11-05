# eng-63a PageIo 增加批量读 `read_page_li`（默认串行 + compio 并发）

## 位置
- `crate/io/fkv_io/src/lib.rs`：`PageIo` trait，`read_page` 之后。
- `crate/io/fkv_io_compio/src/lib.rs`：`impl PageIo for CompioStore`，`read_page` 之后。
- `crate/io/fkv_io_compio/tests/batch.rs`：补测试。

## 方案
1. trait 新增默认实现（不强制改其他后端）：

```rust
/// 批量读页，对齐 C++ `ReadPages`：`page_buf_li[i]` 接收
/// `file_page_id_li[i]` 的整页数据。默认逐页串行，后端可并发提交。
fn read_page_li<'a>(
  &'a self,
  tbl: &'a TableIdent,
  file_page_id_li: &'a [FilePageId],
  page_buf_li: &'a mut [Vec<u8>],
) -> impl Future<Output = Result<()>> + 'a
```

- 长度不一致返回 `KvError::InvalidArgs`；逐页调用 `read_page`。
2. `CompioStore` 覆盖：校验长度后用 `futures::future::try_join_all` 并发提交 `read_page`（一次多 SQE，一次等待）。`futures` 已是依赖。
3. 不新增 trait 方法到其他后端；`MemStore` 用默认串行。
4. 中文注释说明语义（整页覆写、长度契约）。

## 验收
- `cargo check -p fkv_io -p fkv_io_compio`
- `cargo nextest run -p fkv_io`（若有测试）
- compio 测试：多页批量读结果与逐页读一致；长度不匹配报 `InvalidArgs`。
- `cargo clippy -p fkv_io -p fkv_io_compio --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

实现要点：
- `PageIo::read_page_li` 默认实现：长度不一致返回 `KvError::InvalidArgs`，否则逐页 `read_page` 串行。
- `CompioStore` 覆盖：`futures::future::try_join_all` 批内并发提交、一次等待；批大小由 overflow 链每层指针数决定（≤ `overflow_pointers`），天然有界。
- `MemStore` 及测试桩沿用默认实现，无额外改动。

测试文件：`crate/io/fkv_io_compio/tests/batch.rs`（`read_page_li_batch_matches_pages`：批量读与逐页一致、页号/缓冲数量不匹配报 `InvalidArgs`）。

验证：
- `cargo nextest run -p fkv_io_compio --test batch` → 2 passed。
- `cargo clippy -p fkv_io -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings` → 0 warning。

## 复审（2026-11-05 r6）

通过。

- `fkv_io/src/lib.rs:55 PageIo::read_page_li` 默认实现（长度不一致
  `InvalidArgs`，逐页串行）；`fkv_io_compio/src/lib.rs:1991-2008` 覆盖：
  `try_join_all` 批内并发提交 `read_page`、一次等待，注释对齐 C++
  `ReadPages`（`:1989-1990`）；`fkv_io_mem`/`fkv_io_sim` 无覆盖、沿用默认。
- 测试 `tests/batch.rs:63 read_page_li_batch_matches_pages`（批量 vs 逐页一致、
  页号/缓冲数量不匹配 InvalidArgs）。
