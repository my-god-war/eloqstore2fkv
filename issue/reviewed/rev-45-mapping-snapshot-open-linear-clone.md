# rev-45 P2：`MappingSnapshot::open` 每请求 O(chunks) 克隆映射表

关联：`27a-gc-retained-active-mapping-snapshot.md`（快照登记 / GC retained 并集）。

## 位置

- `crate/core/fkv_mapping/src/lib.rs:955-1020`（`MappingSnapshot { core: Rc<SnapshotCore> }`，
  `SnapshotCore.data: MappingTbl`）
- `crate/core/fkv_mapping/src/lib.rs`（`MappingTbl` 为 `Vec<Arc<Chunk>>`，
  `SnapshotRegistry::open` 里 `data: source.data.tbl().clone()`）
- 调用方：`fkv_store` 每个读 / 写 / scan 请求入口 `registry.open(...)`

## 现状

`MappingTbl::clone` 虽共享 `Arc<Chunk>`（页数据不复制），仍要逐 `Chunk` 递增
`Arc` 引用计数并复制指针数组：一次 O(chunks) 的遍历 + 一次按 chunk 数的堆分配。
chunk 数随库容量增长（每个 chunk 覆盖一段逻辑页），大库上单次点读的固定成本
随数据量线性上升；读写热路径每个请求至少一份快照。

## 影响

- 单线程内 `Arc` 原子计数（relaxed）在每 chunk 一次，缓存行乒乓不明显，但
  批量请求下是纯常数放大。
- 分配次数：每请求 1 次 `Vec` 分配 + chunk 数 × `Arc::clone`；与读路径其余
  开销（index cache 命中 + data 页 IO）相比，小 value 点读时占比可观。

## 建议修法（本轮不实现）

1. **快照共享 baseline**：`SnapshotCore` 里保存一份不可变 `MappingTbl`
   引用，未发生 remap 的请求直接复用上一份快照的 `Rc<SnapshotCore>`，
   只替换 `root_id` / 代际（仍 O(1)）。需要 store 侧记录「自上次快照以来
   映射是否变化」（mapper 已有 dirty / changed 页记录，可复用）。
2. **COW 映射表**：`PageMapper` 内部改为持久化结构（`im` / 手写 chunk 级
   路径复制），clone 只复制 root 指针 + 脏 chunk，天然 O(dirty chunks)。
   与 `27-no-cow-read-snapshot` 的方向一致，但改造面覆盖写路径。
3. 短期缓解：`MappingSnapshot::with_root(&snapshot, root_id, ttl_root)` 允许
   同一次请求内多棵树复用同一份快照，避免重复 open。

## 验收

- 大库（chunk 数 ≥ 1e3）点读基准：`SnapshotRegistry::open` 摊薄到纳秒级，
  且不随 chunk 数线性增长；
- `cargo nextest run -p fkv_mapping -p fkv_store`；clippy 零警告；
- GC retained 语义（27a）不变：快照仍持有其可见的完整映射视图。

## 依赖 / 重叠

- 依赖 27a / 27b 已落地；
- 与 `rev-42a`（root meta LRU）、`sto-61b`（多 DS shard 路由）正交。


## DONE（2026-11-05）

**状态：已完成，issue 关闭。**

改动（`crate/core/fkv_mapping/src/lib.rs`；`crate/engine/fkv_store/src/lib.rs`
仅同步快照创建代价文档）：

1. `MappingTbl.chunk_li` 由 `Vec<Arc<Chunk>>` 改为 `Arc<Vec<Arc<Chunk>>>`：
   `clone` 只递增一次引用计数（O(1)），不再逐 chunk 复制指针数组；
   `SnapshotRegistry::open` 的 data / segment 两侧创建代价均摊薄为 O(1)，
   不再随 chunk 数线性增长。
2. 写路径仍 COW：新增 `chunk_li_mut()`，仅在指针表被快照 / clone 共享时
   `Arc::make_mut` 复制一次指针数组；chunk 内容仍按脏 chunk `Arc::make_mut`。
   旧快照视图保持取出时刻的 chunk 集合与内容。
3. `EMPTY_MAPPING_TBL` 改 `LazyLock<MappingTbl>`（`Arc` 字段无法 const 构造）。
4. 注册表登记增量化：`SnapshotRegistry` 增 `prune_len` 阈值，`open` 仅在
   条目数达到阈值时整表摘链一次（摊还 O(1)）；`live_num` / GC retained 仍
   每次精确摘链，27a 语义不变。
5. 诊断 / 测试 API：`chunk_dir_copy_num()`（线程内指针表真实复制次数）、
   `MappingTbl::chunk_dir_share_cnt()`、`SnapshotRegistry::entry_num()`。

测试（`crate/core/fkv_mapping/tests/snapshot.rs`）：

- `snapshot_open_shares_chunk_dir_in_o1`：1000+ chunk 大表连续 64 次 open，
  指针表复制计数为 0，共享计数 = 65，drop 后回到 1；
- `mapper_mutation_copies_shared_chunk_dir_once`：快照存活时首次写只复制一次
  指针表，之后指针表独占不再复制，快照内容保持旧值；
- `registry_registration_is_incremental`：长期 open/drop 1000 次，注册表条目
  始终 ≤ 8（阈值增量摘链）；
- 既有 COW 隔离 / journal / GC retained 用例全绿。

验证：

```
cargo nextest run -p fkv_mapping     # 29 passed
cargo nextest run -p fkv_store       # 143 passed（含 gc_snapshot / concurrent_snapshot）
cargo clippy -p fkv_mapping -p fkv_store --all-targets -- -D warnings   # 0 warning
cargo +nightly fmt -p fkv_mapping -p fkv_store
```

风险 / 备注：

1. `MappingTbl::get` 多一层 `Arc<Vec>` 间接寻址，属共享指针表的常数代价；
   点读热路径未见明显回归（无基准数据，仅结构复核）。
2. 快照存活期间的首次写会复制一次整块指针表 O(chunks)，与旧实现「快照创建时
   O(chunks) 复制」等价（成本从读入口挪到写入口），无净增；无快照时写路径零复制。
3. 未做 chunk 级路径复制（`im` 类持久化结构）：大库高频「读快照跨写」场景
   仍有整表指针复制常数放大；本轮按 P2 验收（open O(1)）收口，后续如需
   写入口也 O(dirty chunks) 可再演进。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 特殊核对「线性 clone 优化后快照语义不变」：

- 结构（`crate/core/fkv_mapping/src/lib.rs`，mtime 07:38 已在 5 分钟写入区外）：
  `MappingTbl.chunk_li: Arc<Vec<Arc<Chunk>>>`（:145）、`chunk_li_mut()` 仅在
  `Arc::strong_count > 1` 时 `Arc::make_mut` 复制一次指针表（:167-171）、
  `chunk_dir_copy_num` / `chunk_dir_share_cnt` 诊断（:105/:175）、`EMPTY_MAPPING_TBL`
  改 `LazyLock`（:997）——`clone` 由 O(chunks) 降为 O(1)，声称成立。
- `SnapshotRegistry::open` 增量摘链：`prune_len` 阈值 + 达到时整表摘链一次
  （:1108/:1140-1142），`entry_num`（:1175）。
- 快照语义不变（亲读 `tests/snapshot.rs`）：`snapshot_open_shares_chunk_dir_in_o1`
  （:99，1000+ chunk 表 64 次 open 复制计数为 0，:116-126 共享数=65、drop 后回 1）；
  `mapper_mutation_copies_shared_chunk_dir_once`（:131，快照存活首次写只复制一次
  :140-146，且 `snapshot.data_tbl().get(page)==old_value` :148——旧视图内容不变，
  即 27a 的「快照持其可见完整映射视图」语义保持）；`registry_registration_is_incremental`
  （:159，1000 次 open/drop 条目 ≤ 8）。
- 与 rev-45「验收：不随 chunk 数线性增长」一致；写路径仍 COW。
