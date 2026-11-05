# core-18a MappingTbl chunk 跨快照空闲池（MappingChunkArena 等价物）

## 来源
`issue/done/core-18-pages-pool-fake-no-reuse-chunk-arena-missing.md` 的残余子项
（审查结论「中期」项）。父 issue 的 PagesPool / deserialize / CompioStore 池化已落地，
本子项单独跟踪。

## 目标
`MappingTbl.chunks: Vec<Arc<Vec<u64>>>` 的 chunk（`CHUNK_SIZE * 8 = 4 KiB`）跨快照复用：
`resize` 收缩 / `MappingTbl` drop / COW `Arc::make_mut` 产生的 chunk 归还池，对齐 C++
`MappingChunkArena::Acquire/Release`（一次批量 `Extend` 1024 个 chunk）。

## 现状与难点
- chunk 是 `Arc<Vec<u64>>`，COW 靠 `Arc::make_mut`；池化须把 chunk 换成带 `Drop` 回池的
  `Arc<Chunk>`，池自身需 `RefCell`/锁，牵动 `MappingTbl` 的 `Clone`/`PartialEq`/`Send`·`Sync`。
- 收益有限：`Arc` 头分配省不掉，池只省每 chunk 一次 4 KiB malloc；缺测量。
- 与 core-11（PageMapper mapping arena / 物理页 retire）同源，宜随该处统一决策。

## 判定
暂缓（中期项）。落地前需先有 chunk 分配打点/基准，证明 allocator churn 是瓶颈；
否则按 rust_review「避免过度设计」不实现。

## 落点（若做）
- `crate/core/fkv_mapping/src/lib.rs`：`MappingTbl` / `Chunk` / `ChunkPool`。
- 需评估 `PageMapper`、`fkv_store`、`fkv_recovery` 的 Send/Sync 与 `PartialEq` 影响。

## DONE（2026-11-04）

实现要点（对齐 C++ `MappingChunkArena`：快照释放的 chunk 归池、下次分配复用）：

- `MappingTbl.chunks` 由 `Vec<Arc<Vec<u64>>>` 改为 `Vec<Arc<Chunk>>`；`Chunk` 实现
  `Drop`（归还线程内池）与 `Clone`（COW 拷贝直接从池取缓冲，不再 `Vec::clone` 新分配）。
- 线程局部无锁池：`CHUNK_POOL: RefCell<Vec<Vec<u64>>>` + `CHUNK_CACHE_MAX = 256`
  （约 1 MiB/线程上限）；`take_chunk`/`give_chunk` 分别在池空/超限时退化为普通分配/释放。
  未命中池的真实分配用 `CHUNK_ALLOC_NUM` 计数（`chunk_alloc_num()` 供测试）。
- `push`/`resize` grow/`deserialize` 全部改走 `Chunk::filled`/池缓冲；`get`/`set`/
  `iter_values` 改读 `chunk.values`。`Arc::make_mut` 的 COW 语义与 `Clone`/`PartialEq` 不变。
- `MappingTbl` 仍 `Send + Sync`（`Chunk` 只含 `Vec<u64>`，无锁字段）。

改动文件：`crate/core/fkv_mapping/src/lib.rs`、`crate/core/fkv_mapping/tests/mapping.rs`。

测试：`cargo nextest run -p fkv_mapping -p fkv_buf` → **22 passed**（新增
`fkv_mapping::mapping::chunk_buffers_are_recycled_across_tables`：建表→释放→重建同尺寸表，
池内 256 块全部命中，仅 `total - CHUNK_CACHE_MAX` 次真实分配；既有
`chunked_mapping_clone_is_cow_and_resize_works` 继续覆盖 COW 语义）。
`cargo clippy -p fkv_mapping --no-deps --all-targets -- -D warnings` 零告警；
`cargo fmt -p fkv_mapping -- --check` 通过；`cargo check -p fkv_root -p fkv_large` 通过。

## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:37-78`（CHUNK_CACHE_MAX=256 线程局部池 take_chunk/give_chunk/chunk_alloc_num）、`:85` struct Chunk、`:98` Clone（从池取缓冲）、`:107` Drop（回池）；测试 `tests/mapping.rs:374 chunk_buffers_are_recycled_across_tables`。
