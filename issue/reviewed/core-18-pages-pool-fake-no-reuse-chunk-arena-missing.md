# core-18 PagesPool 名不副实（allocate 即 new，无回收），MappingTbl COW chunk 分配不池化，AlignedBuf::clone 全量拷贝

## 位置
- Rust: `crate/core/fkv_buf/src/lib.rs:104-123`（`PagesPool` 注释自称"简易页池"，`allocate` 每次 `AlignedBuf::new` 新分配，无 `free/recycle/Release` 路径，`max_cached` 不存在）；`:76-82`（`AlignedBuf::clone` 全量拷贝：`with_align + copy_from_slice`）；`crate/core/fkv_mapping/src/lib.rs:120-128`（`push`：新 chunk 时 `Arc::new(vec![INVALID; 512])`）、`:137-141`（`resize` grow：`Arc::new(vec![INVALID; 512])` per chunk，`Arc::make_mut` 又触发整 chunk 4 KiB 拷贝）
- C++: `eloqstore/include/pool.h:10-83`（`Pool<T>`：`Acquire` 空时新建 + `ReserveHelper::EnsureDefaultReserve`，`Release` 回收进 deque，支持 `max_cached`/`preallocate`）、`eloqstore/include/storage/page_mapper.h:183-223`（`MappingChunkArena::Extend` 一次批量 1024 个 chunk，`Acquire`/`Release` 池化）、`eloqstore/src/storage/page_mapper.cpp:302-319`（`AcquireChunk` 从池取）

## 现状
- `PagesPool` 被谁用？grep 全仓：仅 `fkv_buf` 自身 + `fkv_io` 的 `from_arc` 走 `AlignedBuf::new`，`PagesPool` 基本是 dead API（有 `page_size()`/`allocate()` 两个方法，无人调用 `new` 实例）。真正需要池的是 io 读 buffer 与 mapping chunk。
- `MappingTbl` 的 chunk 生命周期：`Arc<Vec<u64>>` 共享，COW `make_mut` 整块拷贝；`MappingTbl::deserialize`（recovery 冷启动路径）逐 slot `push`，`resize` 分支每次 `Arc::new(vec![...])`——百万 slot 的 mapping 恢复产生约 2000 次 4 KiB 分配 + `vec!` 初始化零填。
- 与 C++ 的差异点不是"错误"而是"无回收通道"：C++ chunk 有 `MappingChunkArena` 池（释放的 snapshot 块归还，重用时零 memset——注意 `AcquireChunk` 拿到的池化 chunk 是脏数据！C++ 靠 `ResizeInternal(init_new_chunks=true)` 显式 fill，Rust `vec![INVALID; 512]` 也做零填）。

## C++ 实现
- `MappingArena`/`MappingChunkArena` 在 `PageManager` 级共享池：所有 `MappingSnapshot` 的 chunk 与 vector 骨架都从池 Acquire/Release。冷启动 replay 不再每次 malloc，快照释放归还。
- `DirectIoBuffer` 的 `EnsureDefaultReserve` 配合 `Pool` 的 `ReserveHelper` 提供"从池取回的 buffer 长度已预置"语义。
- 热路径注意点：C++ chunk 池化后复用必须重新 fill（`InvalidValue`），Rust 的 `Arc::new(vec![...])` 每次都新分配也做 fill，等价但多分配。

## 差异与风险
- 行为无分叉；纯分配器/内存效率问题：
  - recovery 大 mapping 恢复：Rust ≈ 2× 4 KiB 分配/次 COW + 全量零填；C++ chunk 池化后只 memcpy 有效部分（`ResizeInternal(false)` + 逐 chunk memcpy），尾部零填省一半。
  - 每页读 buffer：Rust 每次 `AlignedBuf::new(4096)` 或 `vec![0u8]`（io 层 `read_page` 临时缓冲），C++ 池化 `MemCachedPage` 复用。RSS 碎片化与分配延迟在高 QPS 点读时可见。
- `PagesPool` 死代码：todo.md:174 将其列为"✅ 首版"交付物，实际未接入任何 io 路径——与 core-13 同源（模块未接线），此处只记分配器效率缺口。

## 对齐建议
- 短期：`fkv_io` 层加 `Pool<AlignedBuf>`（Vec 池 + max_cached），`read_page`/`write_page` 从池取归还；`PagesPool` 若保留则补 `recycle(&mut AlignedBuf)`。
- 中期：`MappingTbl` 的 `Arc<Vec<u64>>` 改走 `MappingChunkArena` 等价物——`Vec<Box<Chunk>>` + 回收列表（shard 级，单线程无需锁），push 时优先复用；`resize` grow 的 `Arc::new` 改 `Arc::from(chunk_pool.acquire())`。
- `AlignedBuf::clone` 在 `fkv_io/src/lib.rs:142-151` 被 `FixedBuffer::from_arc(Arc::new(AlignedBuf::new))` 使用——Arc 已防拷贝，clone 分支可 `unreachable!` 或改浅拷贝（但类型是 AlignedBuf 值，浅拷贝会双 free，故 `Arc::clone` 路径不经过此 impl）。

## 性能优化建议
- 恢复大表 mapping 时 `MappingTbl::deserialize` 逐 slot push 的 `Arc::new(vec![0;512])` 改批量：按 chunk 一次性构建 `Vec<u64>` 再 chunk 化（`chunks.chunks_mut`），省掉 `vec![INVALID;512]` 的零填 + `Arc` 分配各一次。

## 严重度
P2（性能/分配器效率；无语义风险）

## 审查结论（2026-11-04）

**采纳（P2 性能项）。**

- 成立：`PagesPool` 是死 API（`allocate` 即新分配、无回收）；`AlignedBuf::clone`
  全量拷贝；`MappingTbl` chunk 无池化、`deserialize` 每 chunk `vec!` 零填。
- 无正确性风险，纯分配/拷贝开销。
- 与 `core-13`（PageCache 未接线）、`io-03`（AlignedBuf 未接 compio）、
  `core-02`（mapping 空闲链）相关，需统一决策 buffer/mapping 池归属。
- 阻塞：`fkv_mapping` 正由 core-02 修改；`fkv_buf` 需与 io-03/io-07 一起改。

## 进展（2026-11-04，部分完成）

- `fkv_buf::PagesPool` 从「allocate 即 new」改为真正的固定页池：
  `free: RefCell<Vec<AlignedBuf>>`、`max_cached`、`allocate` 优先复用、
  新增 `recycle(buf)` 与 `cached()`；大小不匹配 / 超上限直接释放。
- 默认缓存 64 页，可用 `with_max_cached` 调整。
- 新增 `fkv_buf::buf::pool_recycles_and_caps`：复用指针一致、缓存上限生效。
- 验证：`cargo nextest run -p fkv_buf` 4 passed。

未做：`MappingTbl` chunk 池化 / `deserialize` 批量构建；`CompioStore` 读缓冲
仍是非对齐 `Vec`，待 io-21 O_DIRECT 时统一改用 `AlignedBuf` + `PagesPool`。

追加：`MappingTbl::deserialize` 不再逐 slot `push`：
先把 varint 值收集到 `Vec`，再按 `CHUNK_SIZE` 批量构建 `Arc<Vec<u64>>`，
消除每个 slot 的 `Arc::make_mut` 与零填检查。验证：
`cargo nextest run -p fkv_mapping -p fkv_store` 82 passed。

仍未做：chunk 空闲池（跨 snapshot 复用）、`AlignedBuf::clone` 全量拷贝。

追加：`CompioStore` 增加 `direct_pool: HashMap<len, PagesPool>`，
O_DIRECT 读写的 `DirectBuf` 从池取、操作完成后 `recycle`，
不再每次 `AlignedBuf::new`。Linux 交叉验证
`hardening::direct_io_page_roundtrip` 通过。

## 复审（2026-11-04）

已落地（核对当前源码）：

1. `PagesPool` 已是真池：`crate/core/fkv_buf/src/lib.rs` 定义
   `PagesPool { page_size, max_cached, free: RefCell<Vec<AlignedBuf>> }`，
   `allocate` 优先 `pop` 复用、`recycle` 归还（尺寸不符 / 超上限直接释放）、
   `cached()`。测试 `fkv_buf::buf::pool_recycles_and_caps`（复用指针一致 + 上限生效）。
2. `CompioStore` 直接 IO 池：`crate/io/fkv_io_compio/src/lib.rs:426`
   `direct_pool: RefCell<HashMap<usize, PagesPool>>`，`:994 take_direct_buf` /
   `:1003 put_direct_buf` 边上池取还，不再每次 `AlignedBuf::new`。
3. `deserialize` 不再逐 slot `Arc::make_mut`。本轮进一步改为流式构建（见下）。

本轮改动（in-lane）：

- A. `MappingTbl::deserialize` 流式构建：`crate/core/fkv_mapping/src/lib.rs:206-229`。
  不再先收集整表中间 `Vec<u64>`，也不再对满 chunk `vec![INVALID; CHUNK_SIZE]` + 整块
  `copy_from_slice` 覆盖；满 chunk 直接 `take` 移交、仅末尾残块 `resize` 补 `INVALID`。
  省掉一次整表分配/拷贝与满 chunk 的冗余零填（正是原「性能优化建议」的落点）。
  新增边界测试 `fkv_mapping::mapping::deserialize_handles_chunk_boundaries`
  （空表 / `CHUNK_SIZE-1` / `CHUNK_SIZE` / `+1` / `2×`）。
- B. 删除死代码 `impl Clone for AlignedBuf`（`crate/core/fkv_buf/src/lib.rs`）：
  完整拷贝语义，且生产路径零调用——`FixedBuffer` 走 `Arc<AlignedBuf>::clone`，
  全仓 `grep make_mut` 无命中，仅有一个孤儿测试在用它。连带删除
  `fkv_buf::buf::clone_is_independent`。

未采纳的残余项：`MappingTbl` chunk 跨快照空闲池（对应 C++ `MappingChunkArena`）。

- 现状 chunks 为 `Vec<Arc<Vec<u64>>>`，COW 靠 `Arc::make_mut`。做池化须把 chunk 换成
  带 `Drop` 回池的 `Arc<Chunk>`，池自身需 `RefCell`/锁，牵动 `MappingTbl` 的
  `Clone`/`PartialEq`/`Send`·`Sync` 语义。
- 收益有限：`Arc` 头分配省不掉，池只省每 chunk 一次 4 KiB malloc；当前无测量支撑。
- 与 core-11（PageMapper mapping arena / 物理页 retire 链路方向）同源，宜随该处
  统一决策，故本轮不做（`可做但判定为过度设计`，非外部阻塞）。

验证：
- `cargo nextest run -p fkv_mapping -p fkv_buf -p fkv_recovery` → 21 passed。
- `cargo clippy -p fkv_buf -p fkv_mapping --no-deps --all-targets -- -D warnings` 零警告。

备注：复核期间 `crate/core/fkv_mapping/src/lib.rs`（`get`/`set` 签名收敛）与
`tests/mapping.rs` 正被另一会话并发修改；本轮改动落在 `deserialize` 与新增测试，
与其改动行不重叠。

## DONE（2026-11-04）

- 实现要点：`MappingTbl::deserialize` 改流式（无中间整表 `Vec`、无满 chunk 冗余零填）；
  删除生产零调用的 `AlignedBuf::Clone` 全量拷贝实现及其孤儿测试。
- 改动文件：`crate/core/fkv_mapping/src/lib.rs`、`crate/core/fkv_mapping/tests/mapping.rs`、
  `crate/core/fkv_buf/src/lib.rs`、`crate/core/fkv_buf/tests/buf.rs`。
- 测试：`cargo nextest run -p fkv_mapping -p fkv_buf -p fkv_recovery` → 21 passed；
  clippy `-D warnings` 零告警。

## 复审（2026-11-05 r6）

通过。`fkv_buf/src/lib.rs:110-155` PagesPool 真池（max_cached/recycle/cached，allocate 优先复用）；测试 `tests/buf.rs:21 pool_recycles_and_caps`；`fkv_io_compio/src/lib.rs:643` direct_pool + `:1287/:1296` take/put_direct_buf；`fkv_mapping` deserialize 流式构建 + `tests/mapping.rs:290 deserialize_handles_chunk_boundaries`；`AlignedBuf` 的 `Clone` 已删（全仓无 `impl Clone for AlignedBuf`）。chunk 池归 18a。
