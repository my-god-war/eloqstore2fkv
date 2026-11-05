# perf-78a scan 预取 future 栈固定化（消除 dyn / 每页 Box 分配）+ 删冗余 fill(0)

## 位置
- `crate/engine/fkv_task_scan/src/lib.rs:181-200`（预取 future 创建）
- `crate/engine/fkv_task_scan/src/lib.rs:292-318`（`drain_prefetch` / `poll_page_io`）
- `crate/engine/fkv_task_scan/src/lib.rs:165`（cache 命中补读前 `page.fill(0)`）

## 现状
- 预取 future 类型是 `Option<Pin<Box<dyn Future<Output = Result<()>> + '_>>>`，
  每页一次 `Box::pin` 堆分配 + vtable 跳转，违反 `todo.md` 硬约束 4。
- 两个 helper 签名同带 `dyn`。
- `spare.fill(0)` / `page.fill(0)` 后紧接整页 `read_page`，memset 纯冗余。

## 方案
1. 用 `std::pin::pin!(async { ... })` 在栈上固定预取 future：
   分支内 `match next_file_page_id { Some(id) => io.read_page(...).await, None => Ok(()) }`，
   future 类型由 `impl Future` 具体化，零堆分配、无虚调用。
2. 预取 future 的持有域收窄到内层作用域（块），块结束自动释放对 `spare` 的借用，
   之后执行 `swap(&mut page, &mut spare)`；早退路径仍先 `drain_prefetch`。
3. `poll_page_io` / `drain_prefetch` 改为泛型：
   `fn poll_page_io<F: Future<Output = Result<()>>>(fut: Pin<&mut F>, done: &mut Option<Result<()>>)`；
   `drain_prefetch<F: Future<Output = Result<()>>>(fut: Pin<&mut F>, done: Option<Result<()>>)`。
4. 删除 `:165`、`:198` 两处 `fill(0)`（`read_page` 整页覆写语义）。
5. 模块注释保留「数据页单向链表，预取深度上界 1 页」的说明，并补充
   「C++ 6 页预取依赖 index 栈推导，见 eng-65」。

## 约束
- 扫描语义不变：TTL 过滤、分页边界、停止条件、环检测、`LargeValueUnsupported` 行为一致。
- 不引入 `dyn` / 不做 index 栈改造 / 不新增依赖。

## 验收
- `cargo check -p fkv_task_scan` 通过。
- `cargo nextest run -p fkv_task_scan -p fkv_store --no-fail-fast` 全绿。
- `cargo clippy -p fkv_task_scan --no-deps --all-features --all-targets -- -D warnings` 零警告。
- `rg "Box<dyn|dyn Future" crate/engine/fkv_task_scan/src` 无命中。

## DONE（2026-11-04）

### 实现要点
- `crate/engine/fkv_task_scan/src/lib.rs`：预取 future 改为
  `pin!(async { match next_file_page_id { Some(id) => io.read_page(...).await, None => Ok(()) } })`。
  - 类型具体化到 `impl Future`，栈上固定，零堆分配、无 vtable；每页 `Box::pin` 消失。
  - 预取持有域收窄到内层块，块结束释放对 `spare` 的借用后再 `swap`；
    早退路径（large value / decode 错误 / advance 错误）仍先 `drain_prefetch`，
    不取消在飞 compio op。
- `poll_page_io` / `drain_prefetch` 改为泛型 `F: Future<Output = Result<()>>`，
  签名 `Pin<&mut F>`，helper 无 `dyn`、无 `Option` 包裹。
- 删除 `:165`、`:198` 两处 `fill(0)`（`read_page` 整页覆写：mem 写满
  `src.len()==page_size`，compio 写满 `dst[..page_size]`）。
- 模块注释补充预取深度上界 1 页的根因（链式遍历）及 C++ 6 页预取依赖 index 栈（eng-65）。

### 验证
- `cargo check -p fkv_task_scan` 通过；`cargo fmt -p fkv_task_scan -- --check` 通过。
- `cargo nextest run -p fkv_task_scan` 3/3 通过。
- `cargo nextest run -p fkv_store --no-fail-fast` 78/78 通过
  （中途因并发 agent 编译未完成重试两次）。
- `cargo clippy -p fkv_task_scan --no-deps --all-features --all-targets -- -D warnings` 零警告。
- `crate/engine/fkv_task_scan/src` 内已无 `Box<dyn` / `dyn Future`。

### 未采纳（见原 issue 审查结论）
- 预取 6 页：链式遍历下 next id 只能逐页获得，深度上界 1 页；
  解除需改 index 栈推进（eng-65 已否决），且改变扫描语义。
- io_uring 批量提交 / fkv_buf 池化：同受链式遍历与低收益约束，不做。

## 复审（2026-11-05 r6）

通过。

- `fkv_task_scan/src/lib.rs` 全文 `Box<dyn` / `dyn Future` 零命中；
  `:194-197` 预取 future `pin!(async { match next_file_page_id {...} })` 栈上
  固定，注释写明避免每页 Box::pin 堆分配与 dyn 虚调用。
- 冗余 `fill(0)` 已删（src 全文零命中）。
