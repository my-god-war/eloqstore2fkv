# io-03 Compio 读写缓冲双重拷贝、write_page_li 每次分配新 Vec 且未用 write_vectored_at 与 AlignedBuf

## 位置
- Rust: `crate/io/fkv_io_compio/src/lib.rs:218-247`（`BufPool` 用 `vec![0u8; len]` 非对齐缓冲）、`:906-931`（`read_page` 池缓冲读完再 `copy_from_slice` 到 `dst`）、`:959-1010`（`write_page_li` 每段连续页新建 `vec![0u8; count * self.page_size]` 拷贝并串行 `.await`）、`crate/core/fkv_buf/src/lib.rs:12-123`（`AlignedBuf` 未实现 `compio::buf::{IoBuf, IoBufMut}`，`PagesPool::allocate` 无池化每次新分配）
- Compio: `compio/compio-buf/src/io_buf.rs:12-15, 377-422, 595-628`（`IoBuf`、`IoBufMut`、`SetLen`；`Vec<u8>` 作为 `IoBufMut` 直接向未初始化容量 `0..cap` 读入并 `set_len`）、`compio/compio-fs/src/file.rs:260-331`（`read_at`、`read_managed_at`、`write_vectored_at`）
- C++: `eloqstore/include/direct_io_buffer.h:18-190`（4096 对齐 `DirectIoBuffer`）、`eloqstore/src/async_io_manager.cpp:253-370, 1243-1308`（预注册 `PagePool` + `WriteBufferAggregator` 并发提交 SQE）

## 现状
结合 `compio` 源码审查 `fkv_io_compio` 的缓冲管理，存在 4 处反模式：
1. **每次 `read_page` 零初始化 + 双重内存拷贝**：调用方（`fkv_task_read`/`fkv_task_scan`/`fkv_btree`）先分配 `vec![0u8; page_size]`（清零一次）并传 `&mut [u8]` 给 `read_page`；`CompioStore::read_page` 再从 `read_buf` 取一个 `Vec<u8>` 传给 `file.read_at(buf, offset).await`，完成后执行 `dst[..page_size].copy_from_slice(&buf[..page_size])`！实际上 `compio` 的 `IoBufMut` 支持直接向 `Vec::with_capacity(page_size)`（或 `AlignedBuf`）的未初始化内存读入并 `advance_to(read_len)`，无需清零也无需二次 `memcpy`。
2. **`write_page_li` 绕过缓冲池每次分配零初始化大 `Vec` 且不用 `write_vectored_at`**：`write_page_li`（`lib.rs:994`）对每段连续页执行 `let mut buf = vec![0u8; count * self.page_size];` 并逐页 `copy_from_slice`，写完直接丢弃 `buf`！而 `compio::fs::File` 实现了 `AsyncWriteAt::write_vectored_at`（底层 `IORING_OP_WRITEV` / `pwritev`），可直接接收 `Vec<Vec<u8>>` / `Vec<AlignedBuf>` 进行零拷贝聚集写！
3. **`write_page_li` 多段非连续写串行 `.await`**：`while start < sorted.len()` 循环内逐段 `.await`，队列深度恒为 1；而在 `compio` 当前线程 runtime 上，对 `&File` 构造多个 `write_at` / `write_vectored_at` Future 并通过 `try_join_all` 轮询，可将多个 SQE 一次性压入 io_uring 提交环并发执行。
4. **`fkv_buf::AlignedBuf` 与 `compio` 脱节**：`AlignedBuf` 具备 4096 字节对齐，但未实现 `compio::buf::{IoBuf, IoBufMut, SetLen}`，`PagesPool` 也只是每次调 `AlignedBuf::new` 的空壳，导致 `CompioStore` 只能用对齐度为 1 的 `Vec<u8>`，无法开启 Linux `O_DIRECT`。

## C++ 实现
C++ 全链路使用 4096 对齐的 `DirectIoBuffer`（来自 `PagePool` 空闲链表），读操作零拷贝直接落入目标页缓冲，写操作将整棵树批次的页聚合后批量提交给 io_uring，单次 `io_uring_submit_and_wait`。

## 差异与风险
- 每页读写多一次 4 KiB~64 KiB `memcpy` 与清零开销，批量写频繁触发大块堆内存分配/释放（allocator 抖动）。
- 多段页写串行化，NVMe 队列深度无法打满。
- 无法开启 `O_DIRECT`。

## 对齐建议
- 为 `fkv_buf::AlignedBuf` 实现 `compio::buf::{IoBuf, IoBufMut, SetLen}`，并完善 `PagesPool` 的空闲回收链表。
- `write_page_li` 改用 `file.write_vectored_at`（直接将连续段内的页切片/缓冲作为 `IoVectoredBuf` 提交，零额外分配、零拷贝），并用 `futures::future::try_join_all` 并发提交同一批次的所有连续段。
- 为 `PageIo` 增加或改造缓冲所有权接口（如传入/返回 `AlignedBuf` 或 `&mut Vec<u8>` 通过 `std::mem::take` 直传给 `file.read_at`），彻底消除 `read_page` 的二次 `copy_from_slice` 与预清零。

## 性能优化建议
消除每页读写的二次 `memcpy` + `write_vectored_at` 零拷贝聚集写 + 多 SQE 并发提交，是逼近 C++ io_uring 吞吐与 p99 延迟的核心路径。

## 严重度
P1（IO 吞吐与 CPU 拷贝开销）

## 审查结论（2026-11-04）

**总结论：部分采纳，转 `issue/todo/`。**

原文 4 条反模式中，只有“写路径大缓冲不复用 + 串行提交”和“`AlignedBuf` 未接 compio traits”完全属实且可独立落地。

读路径的“每次零初始化”已过时：`BufPool::take_read` 已用 `Vec::with_capacity` 直读未初始化容量，池也已按容量复用。剩下的双拷贝不是疏忽，而是 `PageIo::read_page(dst: &mut [u8])` 与 `IoBuf: 'static` 的硬冲突，必须改缓冲所有权接口才能消除。

`write_vectored_at` 在 compio 0.19.2 确实存在，但 `IoVectoredBuf: 'static`，当前 `&[(FilePageId, Vec<u8>)]` 的借用页切片无法直接提交。“零拷贝聚集写”只能随 `PageIo::write_page_li` 改为按值接收页缓冲后才成立。

`try_join_all` 不能直接用于 `BufResult`；`实现 traits 即可开 O_DIRECT` 也不成立，缺 traits 只是必要条件之一。

### 逐条核验

| # | 原文断言 | 源码核验 | 判定 |
|---|---|---|---|
| 1 | `read_page` 零初始化 + 双重拷贝 | `fkv_io_compio/src/lib.rs:984-1015` 从池取缓冲直读，再 `dst[..page_size].copy_from_slice`。读池 `take_read` 在 `:267-276` 已用 `Vec::with_capacity`，不预清零。池分配计数见 `tests/hardening.rs:106`，测试明确断言“只保留一处 copy”。调用方的 `vec![0u8; page_size]` 属于另一层清零 | **部分属实**。双拷贝属实，原因是 API 约束；读池零初始化不属实 |
| 2 | `BufPool` 用 `vec![0u8; len]` 非对齐缓冲 | `:248-260` 的写池确实 `vec![0u8; len]`；读池 `:267-276` 不是 | **部分属实**，只对写池成立 |
| 3 | `write_page_li` 每段新建大 `Vec` 并逐页拷贝 | `:1043-1092`，每段 `vec![0u8; count * page_size]`，循环 `copy_from_slice`，未复用 `write_buf` 池 | **属实** |
| 4 | `write_page_li` 多段串行 `.await` | 同一函数 `while start < sorted.len()` 内逐段 `write_at(...).await`，队列深度 1 | **属实**。但常见批量是单段连续页，`tests/batch.rs:25-51` 断言 4 页合并为 1 次写，收益集中在跨文件/多空洞批次 |
| 5 | compio 0.19 有 `write_vectored_at` / `IoVectoredBuf` | `compio-fs-0.12.1/src/file.rs:264` 存在 `write_vectored_at<T: IoVectoredBuf>`，底层 `compio-driver` 的 `WriteVectoredAt` 映射到 `Writev`。`compio-buf-0.8.3/src/io_vec_buf.rs:12` 定义 `IoVectoredBuf: 'static`，`:100` 为 `Vec<T>` 实现 | **API 存在属实** |
| 6 | 可直接把页切片作为 `IoVectoredBuf` 零拷贝提交 | 借用切片不能实现 `'static` trait。实现只覆盖 `Vec<T>`、数组、`&'static [T]` 等拥有/静态类型。当前签名只提供 `&Vec<u8>`，无法交给 `write_vectored_at` | **不成立**。必须先把 `page_li` 改为按值接收 |
| 7 | `try_join_all` 可并发提交多个 SQE | `read_at`/`write_at` 返回 `BufResult<usize, T>`，不是 `Result`，`try_join_all` 不适用；应用 `join_all` 后逐个检查。`compio-runtime` 的 `Submit::poll` 首次 poll 即入驱动队列，同一任务内 `join_all` 轮询多个 future 确实能把多个 SQE 压入同一轮提交 | **思路属实，API 写法有误** |
| 8 | `AlignedBuf` 未实现 `IoBuf`/`IoBufMut`/`SetLen` | `fkv_buf/src/lib.rs:14-99` 只有 `Deref`/`DerefMut`，无 compio 依赖。`IoBufMut` 还要求区分已初始化长度与容量，当前结构只有固定 `len` 且 `alloc_zeroed` | **属实**，但不是“加几个 impl”那么小，需要改结构语义 |
| 9 | `PagesPool` 是空壳，无回收 | `fkv_buf/src/lib.rs:106-122`，`allocate` 每次 `AlignedBuf::new` | **属实** |
| 10 | 因此无法开启 `O_DIRECT` | `compio-fs` 的 `OpenOption::custom_flags` 可用（`open_options/unix.rs:55`）。但还需：打开时加 `O_DIRECT`、所有页读/写缓冲 4 KiB 对齐、偏移与长度满足设备块大小、manifest/segment 等所有路径改造、合并写缓冲不能再是普通 `Vec` | **因果不完整**。缺 traits 只是前置条件，不能作为独立修复项 |

### 采纳范围与开发计划

#### A. 立即做，低风险（本次采纳）

1. `write_page_li` 复用写池
   - 文件：`crate/io/fkv_io_compio/src/lib.rs`
   - 把 `let mut buf = vec![0u8; count * self.page_size]` 改为 `self.take_write_buf(count * self.page_size)`，写完 `put_write_buf` 归还，与 `write_page` 一致。
   - 验收：`cargo test -p fkv_io_compio`；新增或扩展测试，重复写同尺寸连续段后 `buf_alloc_num()` 不再增长。注意 `BUF_POOL_LIMIT = 8`，只缓存最近尺寸，避免无上限保留大块。
   - 风险：写池按 `len` 分组，批量段尺寸多样时命中率有限，收益是减少 allocator 抖动，不是消除拷贝。

2. 删除调用方读前冗余清零
   - 位置：`fkv_task_read/src/lib.rs:36,229,242`，`fkv_task_scan/src/lib.rs:158,184`，`fkv_task_write/src/lib.rs:199,291,378,465`
   - `read_page` 成功时覆盖 `dst[..page_size]`，短读直接返回错误且不写 `dst`，调用方 `?` 后不再使用旧数据，因此 `page.fill(0)` / `spare.fill(0)` 可删。
   - 验收：`./test.sh`。保留初始 `vec![0u8; page_size]`，因为 `read_page` 要求 `dst.len() >= page_size`。
   - 风险：极低。必须确认所有删除点都在 `?` 传播错误后立即返回。

#### B. 随写路径接口重构做（本 issue 保留）

3. `write_page_li` 改为按值接收页缓冲，启用聚集写
   - 改动面：`fkv_io` 的 `PageIo::write_page_li` 签名改为 `Vec<(FilePageId, Vec<u8>)>`，同步 `fkv_io_compio`、`fkv_io_mem` 默认实现、`fkv_cloud`、`fkv_metrics`、`fkv_task_write` 调用点和测试。调用方目前调用后都不再使用 `write_li`，移动所有权可行；`CloudStore` 需在写后继续用页内容，按值转发即可。
   - compio 实现：
     - 先按 `(file_id, page_index)` 排序分段，和现有逻辑一致。
     - 每段移动页 `Vec<u8>` 构造 `Vec<Vec<u8>>`，调用 `file.write_vectored_at(pages, offset)`，避免大 `Vec` 与逐页拷贝。
     - 处理部分写：`BufResult` 返回已写字节数，未写完用 `IoVectoredBuf::slice(written)` 继续同一偏移，直到写完。不要假设一次写满。
     - 每段超过 Linux `IOV_MAX`（1024）时按 1024 页切块，否则 `writev` 可能 `EINVAL`。
     - 先解析并打开所有目标文件（`open_write_file` 有 `RefCell` 缓存，不能把借用跨 await 带到并发 future），再对每段构造 future，用 `futures::future::join_all` 并发提交，逐个检查 `BufResult`，失败时返回 `KvError::IoFail`，最后统一 `mark_dirty`。
   - 验收：
     - 现有 `tests/batch.rs` 语义保持：连续页写次数仍为 1。
     - 新增跨两个文件的多段批次测试，断言数据正确、写次数等于段数。
     - 新增超过 1024 页连续段测试，断言分块后数据完整。
     - 新增多段并发测试，至少验证 future 全部完成且错误传播正确。
   - 风险：
     - `writev` 不做原子保证，部分写、崩溃后半写页与现有合并写风险同级，依赖 checksum 检测。
     - 页 `Vec<u8>` 的容量可能大于 `page_size`，`IoBuf::as_init` 只看 `len`，写入长度正确；但仍应断言每页 `len == page_size`。
     - 多段并发会让同一批次同时在途的缓冲数上升，内存峰值约为批次总字节数，和当前一次只保留一段不同。需要在批次规模上限或信号量上做约束。
     - 与 `eng-32` 有重叠：shard 串行请求才是队列深度上不去的根因，单次 `write_page_li` 内并发收益有限。

#### C. 另拆任务，不在本 issue 直接做

4. 读路径去双拷贝
   - 需要把 `PageIo::read_page(dst: &mut [u8])` 改为固定容量、可复用的拥有型页缓冲，例如 `PageBuf`/`AlignedBuf`，由调用方持有并跨请求复用。
   - 理由：`IoBuf: 'static` 禁止借入 `&mut [u8]`；若改成 `&mut Vec<u8>` 再 `mem::take`，compio 按容量读，容量大于 `page_size` 时会越页读，破坏 `dst` 尾部不变语义。
   - 该改动会牵动 `fkv_io`、`fkv_io_mem`、`fkv_io_compio`、`fkv_cloud`、`fkv_metrics`、`fkv_task_read`、`fkv_task_scan`、`fkv_task_write`、`fkv_task_compact` 和全部测试，建议单独 issue，并与 `core-13` 页缓存/swizzle 一起设计。

5. `AlignedBuf` 接 compio traits 与 `PagesPool` 回收
   - 仅当第 4 项落地后才有直接收益。
   - 实现 `IoBuf`/`IoBufMut`/`SetLen` 需要把 `AlignedBuf` 拆成 `cap` 与已初始化 `len`，并新增不清零的 `uninit` 构造；当前 `Deref` 默认整块已初始化，直接对未初始化内存做 `&[u8]` 是 UB。
   - orphan rule 决定 impl 只能放在 `fkv_buf`，因此需要给 `fkv_buf` 增加可选依赖 `compio-buf`（`optional = true`，由 `fkv_io_compio` 开启），避免核心库硬绑运行时。
   - `PagesPool` 回收用 `Rc<RefCell<Vec<AlignedBuf>>>` 或线程内 slab，注意 `AlignedBuf` 固定长度、按页大小分桶。

6. `O_DIRECT`
   - 单独任务，不能由本 issue 顺手完成。
   - 需要 `OpenOption::custom_flags(libc::O_DIRECT)`（仅 Linux）、全链路对齐缓冲、偏移/长度按设备逻辑块对齐、manifest 与 segment 路径分别评估，并做有无页缓存的基准。
   - 现有 `fixed.rs` 的 registered buffer 只覆盖 segment 固定写，和页路径 O_DIRECT 不是同一套。
   - macOS 无 `O_DIRECT`，必须 cfg 隔离。

### 不采纳的原文表述

- “`write_page_li` 可直接把连续段内的页切片作为 `IoVectoredBuf` 提交，零额外分配、零拷贝”：当前 API 下不成立，`IoVectoredBuf: 'static`，借用页切片无法提交。必须先改所有权。
- “用 `try_join_all` 并发提交”：`BufResult` 不是 `Result`，应使用 `join_all` 后逐项检查。
- “为 `AlignedBuf` 实现 compio traits 并完善 `PagesPool`，即可消除 `read_page` 二次拷贝并开启 `O_DIRECT`”：实现 traits 本身不改变 `read_page` 的 `&mut [u8]` 签名，也不等于 O_DIRECT 可用。
- “每页读写多一次 4 KiB~64 KiB memcpy 与清零开销”：读路径的清零已由 `Vec::with_capacity` 消除；剩余成本是单次 `copy_from_slice`。写路径的清零来自 `vec![0u8; len]`，改池后可复用，但仍有一次页到连续缓冲的拷贝，除非完成 B 项。

### 结论

采纳 A 项并保留 B 项作为本 issue 的后续实现；C 项拆成独立任务。

原始严重度维持 P1，但应改为“写路径分配抖动 + 批量写单队列深度”，读路径双拷贝降为 P2 并依赖接口重构。

## 阻塞：与并发会话文件冲突（2026-11-04）

结论小节已定「部分采纳」，采纳范围 A1/A2 与保留项 B 的落点全部落在本 lane 禁改/超范围文件：

- A1「`write_page_li` 复用写池」：改 `crate/io/fkv_io_compio/src/lib.rs` —— **禁改热点文件**。
- A2「删除调用方读前冗余清零」：改 `crate/engine/fkv_task_read/src/lib.rs`、`fkv_task_scan/src/lib.rs`
  （**禁改热点文件**）与 `crate/engine/fkv_task_write/src/lib.rs`（不在本 lane 允许的 crate 列表）。
- B「`write_page_li` 改按值接收 + `write_vectored_at` + 多段并发」：牵连
  `fkv_io` / `fkv_io_compio` / `fkv_io_mem` / `fkv_cloud` / `fkv_metrics` / `fkv_task_write`
  的 `PageIo::write_page_li` 签名，其中 `fkv_io`、`fkv_io_compio` 为禁改热点，其余超出本 lane。
- C 项（读路径去双拷贝、`AlignedBuf` 接 compio traits、`O_DIRECT`）结论已要求另拆独立 issue，
  非本 lane 范围。

本 lane 允许的 crate 仅 `fkv_object` / `fkv_object_s3` / `fkv_io_mem` / `fkv_io_sim` / `fkv_buf`，
无法覆盖 A1/A2/B 任一落点，故按 lane 约定直接阻塞，不硬改禁改文件。

已试命令：通读结论表格与 `fkv_io_compio/src/lib.rs` / `fkv_io/src/lib.rs` / `fkv_buf/src/lib.rs`
相关实现核对（未改动任何文件）。

需要什么：由持有 `crate/io/fkv_io_compio/src/lib.rs`、`fkv_task_read`、`fkv_task_scan` 写入权的
会话执行 A1/A2；B 与 C 项在解锁 `fkv_io/src/lib.rs` 后按各自 issue 推进。

## DONE（2026-11-04）

### A 项（已完成）

1. **`write_page_li` 段缓冲复用写池**：核对确认 `write_page_group`（`fkv_io_compio/src/lib.rs`）已用
   `take_write_buf(bytes)` 取池缓冲、`write_chunk` 写完 `put_write_buf` 归还；Linux 优先走注册写槽位，
   非 Linux / 注册不可用回落普通写池。本次未改实现，只补测试
   `tests/write_pool.rs::repeated_batches_reuse_write_bufs`：96 页按 18 页分组（5 个满组 + 1 个 6 页尾组）
   重复写 5 批，断言 `buf_alloc_num()` 不再增长（Linux 走注册槽位时为 0，同样成立）。
2. **删除调用方读前冗余 `fill(0)`**：`PageIo::read_page` 契约明确“后端须整页覆写”，删除点均在
   `read_page(...).await?` 之前、失败即 `?` 返回：
   - `fkv_task_read/src/lib.rs`：cache 下降后补读 data page、回退前一页，共 2 处。
   - `fkv_task_write/src/lib.rs`：路由下降、最左下降、overflow 链回收，共 3 处；测试助手 2 处。
   - `fkv_task_scan/src/lib.rs`：现状已无 `fill(0)`，无需改动。
   - 保留 `vec![0u8; page_size]` 分配（`read_page` 要求 `dst.len() >= page_size`）。
   同时删除 `fkv_task_write` 测试中两处读前 `buf.fill(0)`。

### B 项（未做，保留后续）

`write_page_li` 改按值接收页缓冲 + `write_vectored_at` 未实施，原因：

- 需改 `fkv_io::PageIo::write_page_li` 签名（`&[(FilePageId, Vec<u8>)]` → `Vec<(FilePageId, Vec<u8>)>`），
  连带 `fkv_shard/src/backend.rs`、`fkv_runtime` 调用链；这两个 crate 正由并行会话修改，本 lane 禁改。
- 即使改签名，当前 `write_page_li` 已按连续段分组并用 `buffer_unordered(WRITE_INFLIGHT_LIMIT)` 并发提交，
  剩余收益只是省掉“页 → 连续缓冲”的一次拷贝（Linux 固定写槽位下也是一次拷贝），
  且 `write_vectored_at` 需处理部分写、`IOV_MAX` 分块与内存峰值约束，改动面与风险均超出本次保守范围。

后续步骤（解锁 `fkv_shard` / `fkv_runtime` 后）：

1. `fkv_io` 改签名并调整默认实现；`fkv_io_compio` / `fkv_cloud` / `fkv_metrics` / `fkv_task_write` /
   `fkv_task_compact` / `fkv_shard` 同步。
2. compio 段内构造 `Vec<Vec<u8>>`，调 `file.write_vectored_at`，按返回字节数用 `IoVectoredBuf::slice`
   续写；每段超过 1024 页切块。
3. 补测试：跨文件多段、>1024 页连续段、错误传播。

### C 项（明确不做，留独立 issue）

`read_page` 去双拷贝、`AlignedBuf` 实现 compio `IoBuf`/`IoBufMut`/`SetLen`、`O_DIRECT` 全链路启用，
均按本 issue 结论另拆独立任务。

### 验证

- `cargo check`（5 个 crate）：通过。
- `cargo nextest run -p fkv_io -p fkv_io_compio -p fkv_task_read -p fkv_task_scan -p fkv_task_write -p fkv_store --no-fail-fast`：171/171 通过；`fkv_io_compio` 单跑 36/36 通过（含新增写池复用测试）。
- `cargo clippy ... --all-features --all-targets -- -D warnings`：零警告。

## 复审（2026-11-05 r6）

通过（A 项范围内声称全部属实）。

- A1 写池复用：`write_page_group` 普通回落路径 `take_write_buf` 取池缓冲、
  `write_chunk` 写完 `put_write_buf` 归还（`fkv_io_compio/src/lib.rs:1532-1537`、
  `:1470-1475`）；`tests/write_pool.rs:8 repeated_batches_reuse_write_bufs`
  断言重复 5 批 `buf_alloc_num()` 不增长（`:21-25`）。
- A2 调用方冗余清零：`fkv_task_read` / `fkv_task_write` / `fkv_task_scan` 三个
  `src/lib.rs` 全文 grep `fill(0)` 零命中。
- B 项（按值接收 + write_vectored_at）按结论继续保留未做，C 项已由 io-21
  （O_DIRECT + DirectBuf）与 io-27（注册写槽位）以其它形态部分承接，与本条
  记录的拆分决议一致。
