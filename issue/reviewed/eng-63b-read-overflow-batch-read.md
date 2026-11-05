# eng-63b read_overflow 批内并发读 + 去掉逐页 fill(0)

## 位置
- `crate/engine/fkv_task_read/src/lib.rs`：`read_overflow`。
- `crate/engine/fkv_task_read/tests/read.rs`：补 overflow 链测试。

## 方案
1. 每层指针先映射为 `file_page_id_li`，缓冲用 `Vec<Vec<u8>>` 跨层复用（`resize_with`，不整链预分配）：
   - `resize_with(file_page_id_li.len(), || vec![0u8; page_size])`；
   - 删除 `page.fill(0)`（`read_page` 整页覆写语义）。
2. 调 `io.read_page_li(tbl, &file_page_id_li, &mut buf_li).await?`，批内一次等待。
3. 按指针顺序 `extend_from_slice(overflow.value()?)`；下一批指针只取本层最后一页（尾页才有指针），用 `buf_li.last()` 而非 `unwrap`。
4. 语义、盘面字节不变；跨层仍串行（链式依赖）。

## 验收
- `cargo check -p fkv_task_read`
- `cargo nextest run -p fkv_task_read -p fkv_io`
- 新增测试：多批 overflow 链读取值正确（覆盖跨批指针）；单页 overflow。
- `cargo clippy -p fkv_task_read -p fkv_io --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

实现要点：
- `read_overflow` 每层先映射 `file_page_id_li`，`buf_li` 跨层复用（`resize_with`），不整链预分配。
- 改为 `io.read_page_li(...)` 批内一次等待；删除逐页 `fill(0)`（`read_page` 整页覆写语义）。
- 按指针顺序拼接 value；下一批指针只取本层末页，`buf_li.last()` 替代 `unwrap`。
- 盘面字节与跨层串行语义不变。

附带修复（阻塞 clippy）：`read_entry` 因并发 TTL 下推变成 8 参数触发 `too_many_arguments`，把
`ttl_filter_now_ms` 并入 `ReadCtx`（`crate/engine/fkv_task_read/src/lib.rs`），同步适配
`crate/engine/fkv_store/src/lib.rs` 的 4 个调用点与 `read_ctx`。行为等价。

测试文件：`crate/engine/fkv_task_read/tests/read.rs`（`overflow_single_batch`、`overflow_cross_batch_chain`：单批 + 跨批指针链）。

验证：
- `cargo nextest run -p fkv_task_read -p fkv_io` → 4 passed。
- `cargo nextest run -p fkv_io_compio --test batch` → 2 passed。
- `cargo nextest run -p fkv_store` → 69 passed。
- `cargo clippy -p fkv_io -p fkv_task_read -p fkv_store -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings` → 0 warning。

## 复审（2026-11-05 r6b）

通过。

- 批读实现：`crate/engine/fkv_task_read/src/lib.rs:226 read_overflow_budget` → `:255 io.read_page_li(...)`（批内一次等待）；逐页 `fill(0)` 已删（文件内 grep 无命中）。
- 测试断言覆盖修复点：`crate/engine/fkv_task_read/tests/read.rs:189 overflow_single_batch`、`:218 overflow_cross_batch_chain`——后者构造「首批两页、末页带下一批指针、第二批一页收尾」的跨批链（`:233-246`），断言重组值逐字节等于拼接期望（`:256-260`），正是原串行指针链语义。
