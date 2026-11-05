# eng-80 `MappingTbl::set` 恒返 Ok 却被 8 处 `?`/`is_ok` 包裹，`get` 双重边界检查；`Op::ALL` / `OP_NUM` 手工平行表

## 位置

- `crate/core/fkv_mapping/src/lib.rs:111-121`（`get`）：先 `index >= self.len` 返回
  `InvalidArgs`，再 `chunk.get(index & mask).ok_or(InvalidArgs)`；chunk 恒为
  `CHUNK_SIZE` 长（`push`/`resize`/`deserialize` 都填满 512 个 slot），第二次
  `get` 的 `None` 分支不可达。
- 同文件 `:123-132`（`set`）：唯一返回 `Ok(())`，没有错误路径。
  调用方仍写成：
  `rebuild_free_page_li`（`:465`）`.is_ok()`、`abort_batch`（`:530`）`let _`、
  `allocate_page`（`:601`）`let _`、`remap_page`（`:629`）`?`、
  `set_file_page`（`:637`）`?`、`free_page`（`:662`）`?`、
  `crate/engine/fkv_recovery/src/lib.rs:142` / `:156` `?`。
- `crate/foundation/fkv_types/src/lib.rs:81-86`（`shard_index`）：`num_shards == 0`
  时静默返回 0；`Runtime::start_impl` 已拒绝 `num_threads == 0`，`Shard::start`
  也拒绝 `queue_size == 0`，生产路径不会走到 0 shard。
- `crate/foundation/fkv_metrics/src/lib.rs:28-67`：`Op` 枚举 16 个变体、
  `OP_NUM: usize = 16`、`Op::ALL: [Op; OP_NUM]` 再列 16 个；
  新增一个 Op 时编译器不会提醒更新后两者，`snapshot` 里 `op_metrics[*op as usize]`
  可能越界（panic）或漏统计。

## 现状

- `MappingTbl::get` 是每次逻辑页查找的热路径（点读、写路径下降、GC 扫描都走），
  多一次 `Option` 分支与 `ok_or` 构造；既然 `index < self.len` 已保证 chunk 存在，
  且 chunk 长度恒为 `CHUNK_SIZE`，可以直接索引：
  ```rust
  let chunk = &self.chunks[index >> CHUNK_SHIFT];
  chunk[index & (CHUNK_SIZE - 1)]
  ```
  保留唯一一次 `index >= len` 检查即可。
- `set` 的 `Result` 没有任何错误来源，强迫 8 个调用点写 `?`/`is_ok`/`let _`，
  读者会误以为“写入可能失败”。改成 `fn set(&mut self, page_id, value)` 后，
  `rebuild_free_page_li` 的 `if ... is_ok()` 直接变成语句，`abort_batch` 的
  `let _ =`、`remap_page` 的 `?` 全部消失，`set_file_page` / `free_page` 的
  `Result` 只保留真正会失败的 `get`。
- `shard_index` 的 0 分支是防御式兜底：它把“调用方传了 0 shard”这种编程错误
  静默变成“所有表都路由到 0 号 shard”。应改成 `debug_assert!(num_shards > 0)`
  或让构造 `Runtime` 时用 `NonZeroU16` 表达不变量。
- `Op` 的三份平行定义是典型的“运行期手工表”：`OP_NUM` 只用于数组长度与
  `ALL` 的长度，`ALL` 又只用于 snapshot。用宏或 `strum` 的
  `EnumCount`/`EnumIter` 可以做到“加变体即自动进入表”，编译器保证一致。

## C++ 实现

C++ 的 `MappingSnapshot::Get` 在 `mapping_tbl_` 上直接索引（chunk 结构由
`MappingChunkArena` 保证长度），没有 `Result` 包裹；`PageMapper` 的
`GetMapping()->mapping_tbl_.Get(i)` 也不返回错误码。`Op` 手工表是本条附带的
Rust 内部问题，无 C++ 对应物。

## 建议

1. `MappingTbl::set` 改为无返回值；删除 `rebuild_free_page_li` 的 `is_ok`、
   `abort_batch`/`allocate_page` 的 `let _`，`remap_page`/`set_file_page`/`free_page`
   只保留 `tbl.get` 的 `?`。
2. `MappingTbl::get` 删除第二次 `chunk.get(...).ok_or(...)`，直接索引；
   可加 `debug_assert!(self.chunks[index >> CHUNK_SHIFT].len() == CHUNK_SIZE)` 在测试构建
   守不变量，release 零成本。
3. `TableIdent::shard_index` 改 `debug_assert_ne!(num_shards, 0)` + 直接取模；
   或把 `Runtime` 的 `num_threads` 收成 `NonZeroU16`，让类型系统承担不变量。
4. `Op`：用 `strum::EnumCount` + `EnumIter`（或一个 `op_enum!` 宏同时生成枚举、
   `COUNT`、`ALL`、`as_str`），删除手写 `OP_NUM` 与 `ALL`；`OpMetrics` 数组长度用
   `Op::COUNT`。这是编译期表，不是运行期 match。
5. 顺手把 `KvError::as_str` 的字符串表与 `StoreMode` 的数值映射也检查一遍：
   这两处目前是 exhaustive match，编译器能兜底，不需要改。

## 严重度

P3（`set`/`get` 是热路径上的可消除分支；`Op` 手工表在新增变体时有越界/漏统计风险）

## 核验记录（2026-11-04）

`MappingTbl::get` / `set` 与 8 个调用点实读确认；`shard_index` 0 分支与
`Runtime::start_impl` 的 `num_threads == 0` 拒绝实读；`Op` 枚举、`OP_NUM`、`ALL`
三份定义实读。C++ `MappingSnapshot::Get` 直接索引为对照。

## DONE（2026-11-04）

四条建议全部落地：

1. **`MappingTbl::set` 去掉假 `Result`**（`crate/core/fkv_mapping/src/lib.rs`）：
   签名改 `fn set(&mut self, page_id: PageId, value: u64)`。调用点同步清理：
   `rebuild_free_page_li` 的 `is_ok()` 直接顺序执行、`abort_batch` / `allocate_page`
   的 `let _ =` 删除、`remap_page` / `set_file_page` / `free_page` 的 `?` 删除；
   `crate/engine/fkv_recovery/src/lib.rs` 两处 `?` 删除。
2. **`get` 去掉第二次边界检查**：`index >= self.len` 已保证 chunk 存在，
   直接 `Ok(chunk[index & (CHUNK_SIZE - 1)])`，加 `debug_assert_eq!(chunk.len(),
   CHUNK_SIZE)` 在测试构建守不变量（release 零成本）。
3. **`TableIdent::shard_index` fail fast**：删除 `num_shards == 0 → 0` 的静默兜底，
   改 `debug_assert_ne!(num_shards, 0)` + 直接取模（0 会除零 panic）。
   `Runtime::start_impl` 已拒绝 0 线程，生产路径不传 0。原测试
   `shard_index_in_range` 里 `assert_eq!(shard_index(0), 0)` 是这条兜底的契约，
   改为 `shard_index_rejects_zero_shards`（`#[should_panic]`）。
4. **`Op` 单一来源**（`crate/foundation/fkv_metrics/src/lib.rs`）：`op_enum!` 宏
   一次生成枚举、`Op::COUNT`（`[stringify!...].len()` 编译期算出）、`Op::ALL`、
   `as_str`；删除手写 `OP_NUM` 与 16 项 `ALL`，`op_metrics: [OpMetrics; Op::COUNT]`。
   新增变体只改宏调用一处，编译器保证数组长度一致（原来漏改会越界 panic 或漏统计）。
5. 附注：`set_file_page` / `remap_page` 的 `Result` 仍在但已无失败路径（保持签名不动，
   避免牵动 `fkv_store` 等调用方）；`KvError::as_str` / `StoreMode` 是 exhaustive
   match，编译器兜底，按建议不改。

验证：
- `cargo nextest run -p fkv_mapping -p fkv_metrics -p fkv_types -p fkv_recovery` 30 passed
- `cargo clippy -p fkv_mapping -p fkv_metrics -p fkv_types -p fkv_recovery --all-targets
  --no-deps -- -D warnings` 零警告
- 全仓无其它 `OP_NUM` 引用（`rg OP_NUM` 仅本条改动前的手写定义）

## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:174` `pub fn set(&mut self, ..)` 无 Result、`:152` get 单边界；`fkv_metrics/src/lib.rs:33` op_enum! 宏（:63 `ALL: [Op; Op::COUNT]` 编译期联动，手写 OP_NUM 已删）；`fkv_types/src/lib.rs:175` shard_index `debug_assert_ne!` 无静默兜底。
