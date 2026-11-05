# eng-32a 读批并发：连续只读请求共享 `&Store` 用 `join_all` 同轮推进

父 issue：`eng-32-workloop-serialization.md`（workloop 全串行）。本文件是可立即落地、无借用冲突的安全子集。

## 为什么不做 per-request `spawn`

`compio::runtime::spawn<F: Future + 'static>` 要求 `'static`，而 `handle(&mut ShardState, ...)`
与 `Store::put(&mut self)` 都排他借用状态。真并发需要先拆 per-table 打开状态（见 `eng-32b`），
属阻塞项。因此本轮只做「不需要 `'static`、不需要拆状态」的并发形式。

## 做法

1. 主循环收到只读请求（`Get` / `Scan` / `Floor`）后，用 `rx.try_recv()` 连续收集同批只读
   envelope，上限 `MAX_READ_BATCH = 32`；遇到非只读请求立即停止，该 envelope 放回
   `carry` 槽位，保证与后续写请求的先后顺序不变。
2. 批内逐条做准入（`admit`）与 `ensure_table_ready`（需要 `&mut ShardState`），全部按原顺序
   串行完成。
3. 之后只取 `&state.store`，用 `futures::future::join_all` 同时推进多条读链。compio 的
   completion IO 在每条链 `.await` 时提交 SQE 并让出，单线程内多请求 IO 因此重叠。
4. 结果按原顺序回执与记录指标；`OutOfMem` 仍走原有串行重试路径。批内没有写、没有
   `Shutdown`、没有 `maintain_after_write`，语义与串行路径一致（批完成前不处理队尾写）。

## 借用前提（已核验）

- `Store::get/scan_page/floor` 都是 `&self`（`crate/engine/fkv_store/src/lib.rs:406,528,2051`）。
- `index_cache` 的 `RefCell` 借用都不跨 `await`（`fkv_task_read/src/lib.rs:94,104,128,163`、
  `fkv_task_scan/src/lib.rs:132,152`），同线程并发 poll 不会 `borrow_mut` 冲突。
- `CompioStore` 所有 `RefCell`/`Cell` 借用同样短作用域（`fd_cache`、`dir_index_map`、
  `read_buf` 池均为 take/put 成对），`read_page_li` 已用 `try_join_all` 并发读页。
- `GetLarge` 走 `fixed_buffer.as_mut()`，**不进读批**，保留串行路径（fixed buffer 池化见
  `eng-32b` 的 io-07 依赖）。

## 不改的部分

- 写请求、`Reopen`、后台维护、attach/detach/shutdown 仍逐条串行。
- 没有取消语义变化：`join_all` 会把批内 future 全部跑到完成，不 drop。

## 验收

- 32 个并发 `Get` 全部返回正确值，无 panic、无 `AlreadyBorrowed`。
- 读批后面排队的 `Put` 仍按原顺序生效（读批不会越过写）。
- `cargo nextest run -p fkv_shard` 通过；clippy 零警告。

## 风险

- 批内同时驻留的读页缓冲最多 32 份，比串行峰值高，但仍有界（`MAX_READ_BATCH`）。
- 批越大，队尾写等到的延迟越接近「32 条读串行」的旧上限，不劣化。
- 未来做真正 `spawn` 时本批结构可直接换成就绪集合（`eng-32b`）。

## DONE（2026-11-04）

实现内容（`crate/engine/fkv_shard/src/lib.rs`）：

- 拆分 `handle` → `admit`（准入）+ `prepare_table`（ensure）+ `dispatch`，串行语义逐行保持。
- 新增 `is_read_op` / `dispatch_read`：`Get` / `Scan` / `Floor` 只借 `&Store`。
- 主循环：只读首请求后用 `rx.try_recv()` 收集同批只读 envelope（`MAX_READ_BATCH = 32`），
  遇到非只读存入 `carry` 槽位，下一轮优先处理，读批不越过写。
- `run_read_batch`：批内串行 admit/prepare，之后 `join_all` 并发推进；逐条回执、
  记录指标；`OutOfMem` 退回 `handle` 串行重试。
- `record_request` 抽出，串行与读批共用指标/日志。
- 新增依赖 `futures`（`default-features=false, features=["std"]`）。

测试（`crate/engine/fkv_shard/tests/read_batch.rs`）：

- `read_batch_serves_concurrent_gets_and_scan`：32 并发 `Get` + 1 `Scan` 全部正确。
- `read_batch_does_not_lose_queued_writes`：读写交替投递后全部 key 可读。

验证：

- `cargo check -p fkv_shard --all-targets` 通过。
- `cargo nextest run -p fkv_shard -p fkv_runtime -p fkv_store`：103 passed，1 skipped。
- `cargo clippy -p fkv_shard --no-deps --all-features --all-targets -- -D warnings` 零警告。
- `cargo fmt -p fkv_shard -- --check` 通过。

遗留：真正 per-request `spawn`、写并发、优先级让出分别见 `eng-32b` / `eng-33` / `eng-34`；
验收指标见 `eng-32c`。

## 复审（2026-11-05 r6b）

通过。

- `MAX_READ_BATCH = 32`（`crate/engine/fkv_shard/src/lib.rs:59`）；主循环只读批收集（`:702-720`，遇非只读停批）、`is_read_op`（`:1304`）、`dispatch_read` 只借 `&Store`（`:1735`，`:2194-2195` 入口分流）。
- 测试：`crate/engine/fkv_shard/tests/read_batch.rs:110` `read_batch_serves_concurrent_gets_and_scan`（32 并发 Get + Scan，`expect_value` 断言每条返回值 `:101-104`）、`:164` `read_batch_does_not_lose_queued_writes`（读写交替不丢写）。
- 后继 `eng-32b`（per-table open state + read spawn）已在 `issue/reviewed/`，本条「无 spawn 安全子集」定位与代码现状相符。
