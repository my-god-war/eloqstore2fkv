# eng-77 云端缓存每次页 IO 都 `tbl.clone()` 构造查找键，`identity()` 每次拼 key 都 clone branch String

## 位置

`crate/engine/fkv_cloud/src/lib.rs`：

- 缓存键是 `(TableIdent, FileId)`，所有读写都用 `tbl.clone()` 现造一个 owned key：
  `contains` `:218-224`、`touch` `:226-233`、`upsert_resident` `:235-265`（`:239` 建 key）、
  `mark_clean` `:267-278`（`:269`）、`mark_dirty_range` `:280-290`、`take_dirty_ranges` `:292-310`、
  `forget` `:312-317`；segment 侧同样有 `:337,344,353,379,384,393`。
  全文件共 13 处 `tbl.clone()`。
- `identity()` `:159-163` 返回 `(String, u64)`，每次调用都 `identity.branch.clone()`；
  `file_key` `:184-188`、`segment_key` `:200-204`、`manifest_key` `:190-194`
  在每次远端读/写/删除时都走这条路径。
- `evict_table` `:798-824` 通过遍历整个 `file_map` / `segment_map` 并按
  `key.tbl_name == tbl.tbl_name && key.partition_id == tbl.partition_id` 过滤来删除，
  每次 detach 是 O(缓存条目数)。

## 现状

- `TableIdent` 的 `tbl_name` 是 `Box<str>`，`clone()` 是一次堆分配 + 拷贝。云模式下每个页请求：
  - 读命中：`touch`（1 次 clone）；
  - 读未命中：`contains` + `file_key`（另一次 branch String clone）+ `upsert_resident`；
  - 写：`ensure_resident` 内 `contains`/`upsert` + `upsert_resident` + `mark_dirty_range`，
    至少 2~3 次 `TableIdent` clone；
  - segment 路径同构。
  即每页 2~4 次小堆分配，纯粹用于 hash 查找键。
- `identity()` 的 branch clone 发生在每次对象 key 拼接（远端 GET/PUT/DELETE、manifest 上传）上；
  虽然不如 `TableIdent` 频繁，但完全没必要：`FileIdentity` 在 `RefCell` 里，格式化时可借用。
- `(TableIdent, FileId)` 扁平键还让“驱逐某张表”退化成全表扫描；改成按表嵌套后，
  `evict_table` 是删除一个外层条目。

## C++ 实现

`include/async_io_manager.h:824` 的 `PartitionFiles` 以文件为键收在一张表下，
`:1014` 的 `tables_` 是 `unordered_map<TableIdent, PartitionFiles>`：
查找时外层用 `TableIdent` 引用（`Find`/`operator[]` 不要求 owned key），
内层用 `TypedFileId`；不存在“为每次页 IO 复制一份表名”的形态。
`FileKey`（`include/types.h:241-245`）只在需要跨表记录 closed file 时才拼完整键。

## 建议

1. `CacheState` 改为按表嵌套：
   ```rust
   struct TableCache {
     data: HashMap<FileId, FileState>,
     segment: HashMap<FileId, SegmentState>,
   }
   struct CacheState {
     table_map: HashMap<TableIdent, TableCache>,
     clock: u64,
     resident_bytes: u64,
   }
   ```
   `contains`/`touch`/`upsert_*` 先 `table_map.get(tbl)` 借用外层，再操作内层 map；
   只有首次插入新表时 `tbl.clone()` 一次。`evict_table` 变成
   `if let Some(cache) = table_map.remove(tbl) { resident_bytes -= cache.bytes() }`。
2. `identity()` 改为借用：
   ```rust
   fn identity(&self) -> core::cell::Ref<'_, FileIdentity>
   ```
   `file_key` / `segment_key` / `manifest_key` 在函数内持有 `Ref` 并格式化成新 String；
   删除 `(String, u64)` 返回值和 branch clone。`Ref` 是标准库借用守卫，不引入 dyn。
3. 如果不想改缓存结构，至少在 `CacheState` 里用 `hashbrown::HashMap` 的
   `raw_entry`/`get_by` 以 `(&TableIdent, FileId)` 借用查找；但嵌套结构同时解决
   `evict_table` 的全表扫描，优先选嵌套。
4. 为 `evict_table` 加回归测试：只删目标表的 data/segment 条目，`resident_bytes` 精确归零。

## 严重度

P2（云模式每页 2~4 次小堆分配 + detach 全表扫描；云路径尚未经 Runtime 暴露，见 issue 12）

## 核验记录（2026-11-04）

13 处 `tbl.clone()` 与 `identity()` / `file_key` / `segment_key` / `evict_table` 行号按当前源码 grep 实读确认。
C++ `PartitionFiles` / `tables_` 引用自 `include/async_io_manager.h:824,1014` 实读。

## DONE（2026-11-04）

三条建议全部落地（`crate/engine/fkv_cloud/src/lib.rs`）：

1. **缓存改按表嵌套**：新增 `TableCache { data: HashMap<FileId, FileState>,
   segment: HashMap<FileId, SegmentState> }`，`CacheState.table_map:
   HashMap<TableIdent, TableCache>` 取代原来的 `(TableIdent, FileId)` 扁平键。
   `contains`/`touch`/`upsert_*`/`mark_*`/`take_dirty_ranges`/`forget`/`dirty_*_li`
   全部改为 `table_map.get(tbl)` 借用外层再操作内层；只有首次建表（`table_slot`）
   才 clone 一次 `TableIdent`。全文件 `tbl.clone()` 由 13 处降到 3 处，剩下 3 处是
   建表与挑选淘汰候选时必须交出 owned 键的场合。
2. **`identity()` 改借用**：返回 `Ref<'_, FileIdentity>`，`file_key` / `manifest_key`
   / `segment_key` / `remote_file_id_page` 在函数内持有 `Ref` 格式化，删掉
   `(String, u64)` 返回值与每 key 一次的 branch `String` clone。注意 `Ref` 不跨
   `await` 持有（clippy `await_holding_refcell_ref`）。
3. **`evict_table` 不再全表扫描**：`table_map.remove(tbl)` + `cache.bytes()`
   一次减账；另加 `drop_if_empty`，`forget` / `forget_segment` 把表内条目清空后
   顺手删掉外层登记，避免空表常驻。
4. 顺带把淘汰候选选择抽成 `oldest_clean_data` / `oldest_clean_segment`：
   仍按 `(last_used, tbl_name, partition_id, file_id)` 取最小，保持原来的确定性平局
   规则（先按表内最小、再跨表最小，与全局最小等价）。

新增回归测试 `fkv_cloud::cloud::evict_table_drops_only_that_table_cache`：
两张表各 2 个 file / 每 file 2048 B，`evict_table(tbl_a)` 后
`resident_file_count` 4→2、`resident_bytes` 8192→4096，且 `tbl_b` 读仍命中、
记账不变。

验证：`cargo nextest run -p fkv_cloud` 24 passed；
`cargo clippy -p fkv_cloud --all-targets --no-deps -- -D warnings` 零警告；
下游 `cargo check -p fkv_store --lib` 通过。

## 复审（2026-11-05 r6b）

通过。

- 按表嵌套：`TableCache`（`crate/engine/fkv_cloud/src/lib.rs:129`）、`CacheState.table_map: HashMap<TableIdent, TableCache>`（`:284`）；`tbl.clone()` 全文件现 4 处（原 13，DONE 写 3——多出的 1 处系 12d CacheGroup 后续改动引入，均为建表/淘汰候选交 owned 键场合，非回归）。
- `identity()` 借用：`:626` 返回 `Ref<'_, FileIdentity>`，无 `(String, u64)` 返回与 branch clone。
- `evict_table` 非全表扫描：`:1803`；`drop_if_empty`（`:185`，调用点 `:973/:1132/:1256`）。
- 测试：`crate/engine/fkv_cloud/tests/cloud.rs:834 evict_table_drops_only_that_table_cache` 在盘。
