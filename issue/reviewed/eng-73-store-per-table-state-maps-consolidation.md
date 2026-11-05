# eng-73 Store 的按表状态散在 11 个平行 map 里，清理逻辑两处手写；touch_table 在默认配置下仍每写 O(表数)

## 位置

- `crate/engine/fkv_store/src/lib.rs:109-140`：`Store` 同时持有
  `mapper_map`、`segment_mapper_map`、`root_map`、`ttl_root_map`、
  `branch_metadata_map`、`manifest_started`、`manifest_size_map`、
  `ttl_next_map`、`compression_map`、`index_cache_map`、`ttl_pending`
  11 个以 `TableIdent` 为键的容器，外加 `table_order`。
- 同文件 `:233-247`（`evict_table`）：手写 11 行 remove；`:1111-1125`
  （`reset_tables`）：手写 11 行 clear；`:2136-2145`（`drop_table`）再单独清理。
- 打开路径按需插入不同子集：`open_empty` `:260-275` 插 4 个 map；
  `install_replay_state` `:1259-1305` 插 7 个 map；`put_at_inner` `:838-885`
  又按需补 `compression_map`/`index_cache_map`。
- `touch_table` `:278-296`：每次 `position` 线性查找 + `VecDeque::remove`；
  调用点 `:274`（`open_empty`）、`:821`（**每个 put 批次**）、`:1255`（`reopen`）。
- `branch_metadata` `:633-677`：读时 `.get(tbl).cloned()`，写回时再
  `metadata.clone()` 插入，一次写入两次克隆整条 `file_ranges` 链。

## 现状

- “一个表是否已打开 / 它的 root、mapper、manifest 大小、TTL 时间、字典、索引缓存是否一致”这个不变量没有类型承载，完全靠 11 个 map 的同步维护。
- 已经因此出过问题：`eng-55` 记录 `evict_table` / `reset_tables` / `drop_table` 漏清 map 导致旧字典与 IndexCache 残留；当前代码虽然逐项补齐了，但仍是“每加一个 map 就要在 3 处记得清”的结构。
- `touch_table` 在 `max_open_tables == 0`（默认）时也先做 `position` + `remove` + `push_back`；写路径每批都付一次 O(打开表数) 扫描，表多时是纯开销。`max_open_tables > 0` 时 `VecDeque` 的 `remove` 仍是 O(n)。
- `branch_metadata` 的两次 clone 在每次写 manifest 时都发生；`file_ranges` 链随 branch/term 历史增长，克隆成本只增不减。

## C++ 实现

C++ 用 `RootMeta` 把同一张表的状态收在一个结构里：
`include/storage/root_meta.h:90-121` 的 `RootMeta` 同时持有
`root_id_`、`ttl_root_id_`、`mapper_`、`segment_mapper_`、`manifest_size_`、
`next_expire_ts_`、`compression_`、`first_unflushed_fp_id_`；
`RootMetaMgr`（`:123-150`）只维护一个 `TableIdent -> Entry{RootMeta}` 的
LRU 链表。不存在“同一张表在多个 hash map 各存一部分”的形态。

## 建议

1. 引入 `TableState`，把上表 11 个字段收进去：
   ```rust
   struct TableState {
     mapper: Option<PageMapper>,
     segment_mapper: Option<PageMapper>,
     root: PageId,
     ttl_root: PageId,
     branch_metadata: BranchManifestMetadata,
     manifest_started: bool,
     manifest_size: usize,
     next_expire: u64,
     compression: Option<DictCompression>,
     index_cache: RefCell<IndexCache>,
     ttl_pending: bool,
   }
   ```
   `Store` 只保留 `table_map: HashMap<TableIdent, TableState>` 与
   `table_order: VecDeque<TableIdent>`。`open_empty` / `install_replay_state` /
   `put_at_inner` 变成对一个 `TableState` 的字段写入；`evict_table` 变成
   `table_map.remove(tbl)` 一行。
2. `touch_table` 在 `max_open_tables == 0` 时直接返回，不维护顺序；开启上限时再用
   `HashMap<TableIdent, usize>` + 代际号做 O(1) LRU，或至少避免每写一次 `VecDeque::remove`。
3. `branch_metadata` 改 `&mut TableState` 原地更新 `last_mut()`，只在写 manifest 时
   借出引用；删除两次 clone。
4. 迁移可分两步：先建 `TableState` 并把 11 个 map 的访问包成方法（不改调用方语义），
   再逐路径替换。`eng-55` 的清理修复可作为回归测试基线。

## 严重度

P2（结构不变量分散；`touch_table` 在默认配置下是每写 O(表数) 的常数项，表多时可见）

## 核验记录（2026-11-04）

11 个字段、`evict_table` 11 行、`reset_tables` 11 行、`open_empty` / `install_replay_state` 插入子集、
`touch_table` 调用点 `:274/:821/:1249`、`branch_metadata` 两次 clone 均实读确认。
C++ `RootMeta` / `RootMetaMgr` 引用自 `include/storage/root_meta.h:90-150` 实读。

## 审查结论（2026-11-04）

判定：**合理，已按建议 1/2/3 实施**（无重复 issue）。

实读确认（以当前源码为准，eng-64/68/74/75/76/core-22/23 之后）：
- `Store` 仍持 11 个以 `TableIdent` 为键的容器 + `table_order`，`evict_table` / `reset_tables` / `drop_table` 三处手写清理。
- `open_empty` / `install_replay_state` / `put_at_inner` 分别插入不同子集。
- `touch_table` 每次 `position + VecDeque::remove`，默认配置下写路径同样付 O(打开表数)。
- `branch_metadata` 读时 clone 整条 `file_ranges` 链，再 clone 回写。

拆分为 1 个 todo：`issue/todo/eng-73a-table-state-consolidation.md`。

## DONE（2026-11-04）

已由 `eng-73a-table-state-consolidation.md` 实施完成，见该文件的 DONE 记录。

## 复审（2026-11-05 r6b）

通过（父条目收口）。`eng-73a` 同批复核通过：`TableState`（`crate/engine/fkv_store/src/lib.rs:154`）+ 单一 `table_map`（`:206`）在盘，11 个平行 map 与三处手写清理不复存在。
