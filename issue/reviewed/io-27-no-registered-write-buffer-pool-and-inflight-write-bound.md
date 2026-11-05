# io-27 append 写缓冲池与 merged-write 机制缺失：IO 层无注册写槽位、无 in-flight 写背压，合并写只在单次 write_page_li 内且各合并组串行 await

## 位置
- Rust: `crate/io/fkv_io_compio/src/lib.rs:959-1010`（`write_page_li`：每次为每个合并组新分配 `vec![0u8; count*page_size]`，每组单独 `write_at` 并**串行 `.await`**，无注册 buffer、无并发提交；分配/拷贝问题另见 io-03）、`:676-690`（`BufPool` 容量恰好复用、按 `Vec::capacity` 分组，与 C++ 池语义不同，见 io-03）
- C++: `eloqstore/src/async_io_manager.cpp:301-370`（`BootstrapRing` 一次性分配并 `io_uring_register_buffers` 注册 `write_buf_` 池，槽位 `write_buf_size`）、`:564-591`（`AcquireWriteBuffer/ReleaseWriteBuffer` 自由链表 + `WaitingZone` 阻塞等待）、`:1243-1308`（`SubmitMergedWrite`：把合并后的多页拷进注册槽位，**一条** `IOSQE_FIXED_FILE + WRITE_FIXED` SQE 发出）、`:976-1006 / 176-178`（`WriteReqPool/MergedWriteReqPool` 以 `max_inflight_write` 为容量 —— 写请求对象池即天然 in-flight 上限）、`:2083-2116`（完成时 `MergedWriteReq` 统一回调 + 归还槽位）；`write_task.cpp:433-435 FlushAppendWrites` 在 `WaitWrite` 前把不足一批的页刷出
- compio: `compio-driver/src/sys/op/managed/iour.rs`（compio 原生"managed buffer"是 **provided-buffer ring**（`BUFFER_SELECT`），服务于内核发 buffer 的读，与 C++ 用户态写缓冲池思路不同；要复刻 C++ 需复用 `fixed.rs` 的 sparse 表 + 自定义 `FixedWrite`）

## 现状
Rust 的写聚合只存在于 `PageIo::write_page_li` 一次调用内部：把 `(file_id, page_index)` 连续的页拷进一个临时 `Vec`，每组合并成一次 `write_at`，组与组之间串行等待完成。没有注册写 buffer、没有跨批次合并、没有 IO 层 in-flight 写并发上限（上限完全依赖 task 层 `WriteGate`，`todo.md §4.3.5`）。

## C++ 实现
C++ append 模式维护常驻**注册写缓冲池**：
- 页写入先拷入 `AcquireWriteBuffer` 借出的固定 registered 槽位，同一 file 内攒到 `write_buffer_size` 边界或 `FlushAppendWrites`；
- 一次 flush = 一条 `WRITE_FIXED` SQE 落整段连续字节，完成回调统一释放页与槽位；
- 池耗尽时 `WaitingZone` 挂起写任务（IO 层背压）；`WriteReqPool(max_inflight_write)` 限制未完成写请求数。

## 差异与风险
- **SQE 数量与拷贝次数高于 C++**：`write_page_li` 的页通常按 write stack 序到达、不保证连续，合并率低时仍是每页一次 `write_at` + 每页一次拷贝；C++ 恒定 ≤ `bytes/write_buffer_size` 条 SQE。
- **合并组串行 await**：多组之间无 `join_all`，一个 4 页 × 3 组的批量写延迟 = 3 次盘往返之和。
- **无 IO 层 in-flight 上限**：后台 compaction/GC 与前台写并发时，写 IO 在 compio 侧无限排队，SQ 深度（见 io-26）成为隐式限流，p99 不可控；C++ 在 IO 层就有 `max_inflight_write` 背压。
- 每次 flush 分配的临时大 `Vec` 加剧 io-03 的分配/拷贝退化，且内存不 4 KiB 对齐，将来补 O_DIRECT（io-21）会直接 EINVAL。

## 对齐建议
- 在 `CompioStore` 增加按 `write_buffer_size` 定长、经 `fixed.rs` sparse 表注册的写槽位池 + 自由链表；`write_page` 对 append 目标文件优先拷槽位、按边界 flush（自定义 `FixedWrite` 提交）。
- `write_page_li`：合并组改为 `futures_util::try_join_all` 并发提交；合并缓冲从对齐 buffer 池借出（与 io-03 合并修复）。
- 引入 IO 层 in-flight 写门（`compio::sync::Semaphore`，容量 `max_inflight_write`），与 task 层 `WriteGate` 分层：task 层管任务级并发，IO 层管 SQE 级并发。

## 性能优化建议
注册写槽位 + `WRITE_FIXED` 使写路径 CPU 拷贝只剩一次（页 → 槽位），且槽位常驻可复用，消除批量写的堆分配毛刺；批量写吞吐目标 `§7.6 ≥0.85×` 基本依赖本条 + io-03。

## 严重度
P2（性能）

## 审查结论（2026-11-04）

**采纳（P2 性能）**。

- 成立：`write_page_li` 每个合并组新分配大 `Vec`、组间串行 await，无注册写
  槽位、无 IO 层 in-flight 上限。
- 拆两步：① `write_page_li` 合并组 `try_join_all` 并发 + 对齐 buffer 池
  （与 io-03 一起）；② 注册写槽位 + `WRITE_FIXED`（与 io-07 / io-21 一起）。
- IO 层信号量与 task 层 `eng-33` WriteGate 分层，不重复。
- 阻塞：`fkv_io_compio` 正由 io-06 修改。

## 进展（2026-11-04，部分完成）

- `write_page_li` 已从「逐组合并后串行 await」改为：
  先按 `(file_id, page_index)` 连续段切好缓冲，再 `join_all` 并发提交所有合并组。
  单组内部仍是一次连续 `write_at`，失败组返回错误并停止后续 `mark_dirty`。
- 验证：`cargo nextest run -p fkv_io_compio -p fkv_store -p fkv_runtime`
  115 passed（含 long_stress、archive_crond）。
- 未做：注册写槽位 + `WRITE_FIXED`、IO 层 `max_inflight_write` 信号量。
  前者依赖 io-07 槽位回收与 io-21 O_DIRECT 对齐缓冲，后者与 `eng-33` WriteGate 分层。

追加：

- `write_page_li` 的合并组改用 `buffer_unordered(WRITE_INFLIGHT_LIMIT=8)`，
  限制单次批量写同时在飞的写组数。
- `sync_data` 的 fdatasync 改为 `buffer_unordered(FSYNC_INFLIGHT_LIMIT=16)`，
  结果与路径显式配对，避免乱序完成导致清错 dirty 项。
- 验证：`cargo nextest run -p fkv_io_compio -p fkv_store` 96 passed。

## 审查结论（2026-11-04，执行复核）

**采纳，P2。** 逐条核对结果：

- 原「位置」行号已漂移：`write_page_li` 现位于
  `crate/io/fkv_io_compio/src/lib.rs:1723`；`BufPool` 仍在 `:298` 附近，
  由 io-03 负责对齐 / 双拷贝，本次不动。
- C++ 描述属实：`eloqstore/src/async_io_manager.cpp:301-370` 注册写槽位、
  `:564-591 Acquire/ReleaseWriteBuffer`、`:1243-1308 SubmitMergedWrite`
  走 `WRITE_FIXED`，`max_inflight_write` 默认 32K 作请求池容量。
- 与 io-36 不重复：io-36 只加「单组合并页数上限」，本次补注册槽位与
  IO 层 SQE 并发上限。
- 前序已落地部分不再重做：合并组 `buffer_unordered(8)` 与 `sync_data`
  并发 fdatasync 保持原样。

落地范围：

1. 注册写槽位池：`FixedWritePool`（`:479`）+ `take_fixed_slot`（`:1142`），
   槽位定长 `max_write_batch_pages * page_size`，复用现有
   `FixedBuffer` / `fixed.rs` sparse 注册表，不造平行机制。
2. `write_page_group`（`:1254`）优先借槽位并提交 `WRITE_FIXED`；
   注册不可用、运行时重建或直接 IO 时回落原池化 / `DirectBuf` 路径。
3. IO 层闸门：`WriteGate`（`:424`，容量 `MAX_INFLIGHT_WRITE = 16`，`:57`）
   限制所有 `write_page_li` 调用叠加后的在飞写组数，与 task 层 WriteGate 分层。
4. 失败语义修正：成功组先 `mark_dirty`，再返回首个错误，避免失败组跳过
   其它已落盘文件的 fdatasync。

不做的部分（保持 issue 关闭范围）：

- 跨 `write_page_li` 的 append 攒批 / `FlushAppendWrites` 需要 task 层接口改动，
  单独跟进。
- 单页 `write_page` 仍走普通池化写：单页无法减少 SQE 数，注册槽位收益低。
- IO 闸门取 16 而非 C++ 默认 32K：`MAX_FIXED_BUFFERS = 64`，需给 segment /
  large value 的 fixed buffer 留槽位；后续可按配置化提升。

验收：

- `crate/io/fkv_io_compio/tests/write_pool.rs` 验证重复批次注册次数不增长
  （非 Linux 恒为 0 也成立），并逐页读回校验。
- 现有 `tests/batch.rs` 的 group 数与 `write_at_num` 语义不变。

## TODO（2026-11-04）

1. `FixedWritePool` + `take_fixed_slot` / `put_fixed_slot`，按长度复用
   `FixedBuffer`，复用 `fixed.rs` 注册表。
2. `write_page_group` 走 `WRITE_FIXED`，保留 direct / 注册失败回落。
3. `WriteGate` 限制 IO 层在飞写组，组 future 内持许可。
4. 回归：`tests/write_pool.rs`、`tests/batch.rs`、`tests/fixed.rs`。

## DONE（2026-11-04）

实现要点：

- `crate/io/fkv_io_compio/src/lib.rs`
  - 新增 `MAX_INFLIGHT_WRITE = 16`、`WriteGate` / `WritePermit`（waker 队列，
    无嵌套调度器、无 block_on）。
  - 新增 `FixedWritePool`，槽位复用现有 `FixedBuffer`；
    `take_fixed_slot` 池满释放异长空闲槽位，注册失败返回 `None` 回落。
  - 新增 `write_page_group`：借槽位 → 页拷入 → `FixedWrite` → 归还槽位；
    回落路径改用 `take_write_buf` / `DirectBuf` 池，不再 `vec![0u8; bytes]`。
  - `write_page_li` 组 future 持 `WriteGate` 许可；成功组先记脏，返回首个错误。
  - 新增 `fixed_write_alloc_num()` 诊断计数。
- `crate/io/fkv_io_compio/tests/write_pool.rs`：96 页 × 5 批，后续批次
  槽位注册次数不变，逐页读回一致。

验证命令与结果：

- `cargo check -p fkv_io_compio`：通过
- `cargo check -p fkv_io_compio --target x86_64-unknown-linux-gnu`：通过（覆盖 Linux fixed 路径）
- `cargo nextest run -p fkv_io_compio`：31 passed
- `cargo clippy -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings`：0 warning
- `cargo clippy ... --target x86_64-unknown-linux-gnu`：0 warning

## 复审（2026-11-05 r6）

通过。

- `WriteGate`/`WritePermit`（`fkv_io_compio/src/lib.rs:554-607`）；组 future
  `write_gate.acquire().await`（`:2088`）；成功组先 `mark_dirty` 再返回首个错误
  （`:2106-2115`）。
- `FixedWritePool`（`:616-622`）、`take_fixed_slot`（`:1348-1386`，含异长槽位让位
  与注册失败回落）、`write_page_group` Linux 优先 `WRITE_FIXED`、回落
  DirectBuf/写池（`:1489-1540`）、`fixed_write_alloc_num()`（`:955-962`）。
- `tests/write_pool.rs:29 repeated_batches_reuse_registered_write_slots` 断言重复
  批次注册次数不增长并逐页读回；`:64 inflight_write_limit_is_configurable`
  覆盖后续演进。
- DONE 中 `MAX_INFLIGHT_WRITE = 16` 已被 io-40 演进为
  `DEFAULT_MAX_INFLIGHT_WRITE = 32 << 10` 并接配置（`:57-60`、`:864-868`）、
  `buffer_unordered` 窗口同值（`:2099-2103`），属后续 issue 正式修正，
  非本条不实；`FIXED_WRITE_SLOT_LIMIT=16` 明确为槽位缓存上限非在飞上限（`:61-67`）。
