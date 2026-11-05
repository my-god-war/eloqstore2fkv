# core-12 AppendAllocator 缺少 hole 统计（SpaceSize/UpdateStat/MinFileId），空尾文件与空间放大判定无依据

## 位置
- Rust: `crate/core/fkv_mapping/src/lib.rs:212-241`（`AppendAllocator` 只有 `max_fp_id`；`free` 为 no-op）、`crate/core/fkv_mapping/src/lib.rs:269-273`（`PooledFilePages::reserve_until` 仅 `retain` free 表）；grep `SpaceSize|space_size|min_file|empty_file|hole` 在 `crate/engine/fkv_gc`、`fkv_task_gc`、`fkv_task_compact`、`fkv_compaction` 全部无命中
- C++: `eloqstore/include/storage/page_mapper.h:285-328`（`min_file_id_`/`empty_file_cnt_`/`SpaceSize()`/`UpdateStat()`/`MinFileId()`）、`eloqstore/src/storage/page_mapper.cpp:660-694`（实现，含空尾文件把 `max_fp_id_` 向上 snap 到文件边界的分支）、使用处 `eloqstore/src/tasks/background_write.cpp:122-127`（`SpaceSize()==0` 判整文件空）、`:195-208`（tail 全空时 `UpdateStat(next_file,0)` 防 compaction 复触发）、`:324-326`

## 现状
Rust `AppendAllocator` 只追踪 `max_fp_id`。删除/重写产生的 hole（文件内不再被 mapping 引用的页）在分配器层没有任何统计，上层 GC/compaction 也无消费端。

## C++ 实现
`AppendAllocator` 维护两个派生统计：`min_file_id_`（最老非空文件）与 `empty_file_cnt_`（其后空文件数），`SpaceSize() = (max_fp_id - min_fp_id<<shift) - empty_cnt<<shift`，供 compaction 判定：
1. 某文件全部页死掉（`SpaceSize==0`）→ 列入删除候选；
2. tail 文件整体空 → `UpdateStat` 把 `max_fp_id_` 向上对齐到新文件边界，避免空尾页反复触发 compaction、并让下一次 `Allocate` 开新文件。

## 差异与风险
- fkv 的 compaction/GC（M3）只能拿 `max_fp_id` + retained set 做全量扫描近似，缺 O(1) 的 hole 比例与「tail 全空」信号 → 空尾文件常驻磁盘、compaction 触发阈值对齐不了 C++ 的行为（C++ `UpdateStat` 的 assert 链 `min_file_id >= 旧值` 也保证单调）。
- 行为不会损坏数据，但空间放大与后台写节奏和基线不一致。
- 注意 C++ `UpdateStat` 的 snap-up 分支（`page_mapper.cpp:671-678`）会改写 `max_fp_id_`：Rust 的 `reserve_until`（`:278`）语义是「单调取 max」，方向一致，缺少的是「谁在什么时机调用它」。

## 对齐建议
- `AppendAllocator` 增补 `min_file_id: FileId`、`empty_file_cnt: u32` 与 `space_size()`/`update_stat(min_file, hole_cnt)`/`min_file_id()`，并在 compact/gc 任务完成文件清理后回写。
- `PooledFilePages` 侧对齐 C++ `Free(std::vector<FilePageId>)` 的批量接口（当前逐 id `free`，compact 释放整页链时循环 push）。

## 性能优化建议
- 统计派生自 GC 每轮扫描的结果（与 C++ 相同：统计是 GC 的产出而非分配器实时计数），分配热路径零额外分支。

## 严重度
P2（功能缺失/性能，M3 GC 落地前不暴露）

## 审查结论（2026-11-04）

### 总结论

**部分采纳**，移入 `issue/todo/`，但需按下方重定范围实施。

成立的骨架：`AppendAllocator` 确实只有 `max_fp_id`，`free` 是 no-op，Rust 侧没有
`SpaceSize` / `min_file_id` / `empty_file_cnt` 任何形态，C++ 对照行号与 snap-up 分支属实。

三处必须修正：

1. **「空尾文件常驻磁盘」不成立**。`fkv_task_gc::gc_files` 按 retained set 删盘上文件，
   零引用文件（含尾文件）会被删，`Store::maintain` 也是 `compact_if_needed` + `gc`。
   真正缺的是删完之后 `max_fp_id` 不前进，下一次 `allocate` 把同一个 file id 重新写出来，
   以及 `truncate` 把过期 `max_fp_id` 写进 manifest 与 branch file range。
   即「陈旧记账」，不是「不删文件」。
2. **「compaction 触发阈值对齐不了」表述不准**。Rust 已有按文件放大率触发：
   `file_live_count` + `should_compact` + `pick_compact_file`，`file_amplify_factor` 默认 2。
   C++ 是全局 `SpaceSize / MappingCount`，Rust 是逐文件最差者，这是两种设计差异，
   不是单纯少一个统计。补上 `space_size()` 后的正当用途是 O(1) 空盘快路径与后续空间上限，
   不是替换现阈值。
3. **消费端缺失的主因不是统计，而是触发链**。`Shard::handle` 的 Put 只调 `store.put`，
   从不调 `put_maintained` / `maintain`，该问题已由 `eng-56` 跟踪。
   本 issue 单独落地不产生任何线上行为变化，必须先说清这一点。

另外两处补充：`PageMapper` 已在 `remap_page`/`free_page` 记录 hole id
（`garbage_file_page_li`），但 `truncate` 处直接丢弃，无计数也无派生。C++ replay
是从 mapping 反推 `min_file_id`/`empty_file_cnt`（`replayer.cpp:358-390`），
Rust `with_tbl` 什么都没反推，这是比「GC 回写」更省事、更贴合基线的入手点。

### 逐条核验表

| 论断 | 源码证据（文件:行） | 判定 |
| --- | --- | --- |
| `AppendAllocator` 只有 `max_fp_id`，`free` 为 no-op | `crate/core/fkv_mapping/src/lib.rs:212-237`，字段 `:213`，`free` `:227` | 成立（原文 212-241 尾部行号偏 4 行） |
| `PooledFilePages::reserve_until` 只 `retain` free 表 | `crate/core/fkv_mapping/src/lib.rs:274-277` | 成立（原文 269-273 偏 5 行） |
| `fkv_gc` / `fkv_task_gc` / `fkv_task_compact` / `fkv_compaction` 无 `SpaceSize`/`space_size`/`min_file`/`empty_file`/`hole` | 全仓 rg 仅命中本 issue 与 `core-14` | 成立 |
| C++ `AppendAllocator` 有 `min_file_id_`/`empty_file_cnt_`/`SpaceSize`/`UpdateStat`/`MinFileId` | `eloqstore/include/storage/page_mapper.h:285-328`，声明 `:307-308`，字段 `:322-327` | 成立 |
| `UpdateStat` 含空尾 snap-up 分支 | `eloqstore/src/storage/page_mapper.cpp:665-681`，snap 在 `:668-678` | 成立 |
| 使用处 `SpaceSize()==0` 判空、tail 全空回写、扫描末尾 `UpdateStat` | `background_write.cpp:121-127`、`:198-208`、`:324-326` | 成立 |
| issue 漏掉的更重要消费端：写路径放大判定 | `write_task.cpp:911-941` `MapperExceedsAmplification`：`space_size >= PagesPerFile && space/mapping > factor`，调用 `background_write.cpp:538-540`、`:905-908` | 成立，issue 未列 |
| 「空尾文件常驻磁盘」 | `fkv_task_gc/src/lib.rs:24-46` 无 retained 即删，`:127-160` branch/archive 版同理；`fkv_store/src/lib.rs:2160-2170` `maintain` 含 `gc` | 不成立。文件会被删，残留的是 `max_fp_id` 记账 |
| 「compaction 触发阈值对齐不了 C++」 | `fkv_gc/src/lib.rs:27-73` 已有 `file_amplification`/`should_compact`/`pick_compact_file`；`fkv_config/src/lib.rs:107` 默认 `file_amplify_factor: 2`；`fkv_store/src/lib.rs:2143-2170` | 部分不成立。差异是全局比例 vs 逐文件最差，非缺统计 |
| Rust 无全局空间统计，O(1) 判定不可得 | `rg space_size` 零命中。`compact_if_needed` 每次都做 `file_live_count` 全 mapping 扫描 + `list_file_id_li` 目录扫描 | 成立 |
| hole 完全无统计 | `fkv_mapping/src/lib.rs:463,494` 记入 `garbage_file_page_li`，`:435-437` 取出，`fkv_store/src/lib.rs:1314,1321` 取出即丢 | 部分成立。有 id 列表，无计数/派生，且被丢弃 |
| 统计应派生自扫描而非实时计数 | C++ `replayer.cpp:358-390` 从 `using_fp_ids` 反推；`with_tbl`（`fkv_mapping/src/lib.rs:356-367`）不反推 | 成立，且是最省事的接入点 |
| `truncate` 保留陈旧 `max_fp_id` | `fkv_store/src/lib.rs:1314-1340`，`max_fp_id: mapper.max_fp_id()` 写入 manifest，`branch_metadata()` `:650-660` 用它算 `max_file_id` | 成立，C++ 对应分支会 snap |
| `create_branch` 已按文件边界 snap | `fkv_store/src/lib.rs:1613-1616` | 成立，说明 snap 语义与现有代码同向，不引入新概念 |
| 应对齐 C++ 批量 `Free(vector)` | `page_mapper.cpp:713-716` 仅 `free_ids_.insert(end, ...)`，与逐个 push 语义相同；Pooled 生产不可达（reopen 硬编码 Append） | 不成立/无收益 |
| issue 定位行号 | 仅上述两处 Rust 行号偏移 4-5 行，其余准确 | 基本成立 |

### 采纳后的开发计划

#### 阶段 1：`fkv_mapping` 统计与派生

1. `AppendAllocator` 增加 `min_file_id: FileId`、`empty_file_cnt: u32`。
   - `new` 二者取 0。
   - `space_size(&self, pages_per_file_shift: u8) -> u64`：
     `(max_fp_id - (min_file_id << shift)) - (empty_file_cnt << shift)`，release 下用
     `saturating_sub`，不加 panic 断言。
   - `update_stat(&mut self, min_file_id: FileId, empty_file_cnt: u32, shift: u8)`：
     `debug_assert!(min_file_id >= self.min_file_id)`，取 `max` 保证单调，
     若 `(min_file_id << shift) > max_fp_id` 则 snap `max_fp_id` 到该边界，
     最后回写两个字段。
   - 保留 `free` no-op 语义。
2. `FilePageAllocator` 增加 `space_size(shift)` / `update_stat(...)` / `min_file_id()` 转发。
   Pooled 分支 `update_stat` 用 `debug_assert!` 标记不可达，`space_size` 返回
   `free_cnt` 无关值（Pooled 无 C++ 对照，不实现假语义，直接 `debug_assert_unreachable`
   风格返回 0 并在文档注明仅 Append 有效）。
3. `MappingTbl` 增加一次遍历的派生函数（避免逻辑重复，供 `with_tbl` 与 Store 共用）：
   `fn file_stat(&self, shift: u8) -> (Option<FileId>, u32, FilePageId)`
   - 返回首个有 live page 的 file id、相邻占用文件之间的空洞文件数、最大 live fp id。
   - 顺序：`iter_values()` 过滤 `is_file_page_id`，`Option<FileId>` 记录上界，
     `file_id > prev + 1` 累加 `file_id - prev - 1`。
   对齐 `replayer.cpp:373-389` 的空洞计数方式。
4. `PageMapper::with_tbl` 在 `AllocMode::Append` 下调用 `file_stat` 并 `update_stat`
   （快照 max 取 `max_fp_id.max(max_live + 1)`），不新增 manifest 字段。
   `PageMapper` 增加 `space_size(shift)` / `min_file_id()` / `update_stat_by_scan(shift)`。
5. `PageMapper::wipe_tail(&mut self, shift: u8)`：`space_size > 0` 时
   `next_file = (max_fp_id + mask) >> shift`，`update_stat(next_file, 0, shift)`。
   仅在调用方确认「本表盘上已无任何 retained 文件」时使用。
6. 回滚日志无需改动：`BatchJournal` 已整体 clone `allocator`，新增字段自动被快照。

#### 阶段 2：`fkv_store` 接线

1. `Store::truncate`（`:1294-1340`）：清空 data / segment 后、构造 manifest 前，
   对两个 mapper 调 `wipe_tail`（data 用 `pages_per_file_shift`，segment 用
   `segments_per_file_shift`），使落盘 `max_fp_id` 与 `branch_metadata.file_ranges` 不再过期。
2. `Store::compact_if_needed`（`:2143-2170`）：在 `file_live_count` 之前加
   `if mapper.space_size(shift) == 0 { return Ok(None) }`，空盘写批次免掉全 mapping 扫描
   与目录列举。这是本 issue 唯一的 O(1) 收益点。
3. `Store::maintain`（`:2160-2170`）：`compact_if_needed` + `gc` 之后，若本次 `gc`
   返回删除数等于盘上文件数且 mapping 已空，调 `wipe_tail`。保守写法：
   `gc` 后 `list_file_id_li` 为空才 snap，避免误伤其他 branch retained。
4. `compact_if_needed` 选中并 compact 成功后，用已有的 `live_map` 重算
   `min_file_id`/`empty_file_cnt` 并 `update_stat`，避免多扫一遍。
5. 不改 `pick_compact_file` 跳过 `live == 0` 的行为（空文件由 gc 删），
   不改全局阈值语义（见总结论第 2 点）。
6. 不对齐 C++ 批量 `Free`。

#### 阶段 3：测试与验收

- `crate/core/fkv_mapping/tests/mapping.rs`：
  - `append_space_size_and_snap_up`：alloc 到 10、free、`update_stat(2,0)`，
    断言 `space_size` 与 `max_fp_id` 按 C++ 公式对齐，snap 分支生效。
  - `with_tbl_derives_space_stat`：构造含空洞文件的 `MappingTbl`，
    断言 `min_file_id`/`empty_file_cnt`/`space_size`。
  - `update_stat_is_monotonic`：回退 `min_file_id` 不生效。
- `crate/engine/fkv_store/tests/`：
  - `truncate_snaps_tail_max_fp_id`：truncate 后 `max_fp_id % pages_per_file == 0`
    且 manifest `max_fp_id` 同步。
  - `compact_if_needed_skips_scan_when_space_empty`：空盘时返回 `None`，
    可用注入 IO 统计 `list_file_id_li` 调用次数（若接口允许），否则以行为断言代替。
- 回归：`branch_isolation.rs`、`gc_store.rs`、`fkv_task_gc/tests/gc.rs`
  现有精确 fp 断言需逐条复核。
- 验收：`./sh/clippy.sh` 零警告，`./test.sh` 全通过。

### 风险

- **snap-up 改变 manifest / file range 的绝对值**。`branch_isolation.rs:95-118,263-275`
  与 `branch_archive.rs:64` 有精确 `max_fp_id` 断言，snap 会使其偏移一个文件边界，
  必须同步修断言并说明是期望行为。
- **多 branch retained 时误 snap**。`Store::gc` 用 active + branch + archive 的并集，
  只有在「盘上文件全空」时 snap 才安全。`truncate` 只清当前 branch 的 mapping，
  若父 branch 仍引用同表文件则不能 snap，实现时需按 `branch_metadata.file_ranges`
  判断是否有其他 branch 覆盖该表，不能无条件调 `wipe_tail`。
- **`min_file_id` 单调断言**。compaction 只会把 live 页往后搬，逻辑上单调。
  但 `truncate` 后 `min_file_id` 会跳到 `max_fp_id` 边界，接着又有新分配时
  派生值可能小于已 snap 值，因此只能用 `max` 保护，禁止 release panic。
- **`space_size == 0` 快路径的正确性**依赖 `space_size >= live 数` 不变式。
  派生函数必须只统计 `is_file_page_id`，漏掉会导致 compaction 永久不触发。
- **不解决触发链**。本 issue 落地后若 `eng-56` 未完成，线上仍不会自动 compact/GC，
  预期收益为零，只提供正确的记账能力。

## 进展（2026-11-04，部分完成）

- `AppendAllocator` 增加 `min_file_id` / `empty_file_cnt`：
  - `space_size(pages_per_file_shift)` 按文件边界扣除空文件，饱和返回 0；
  - `update_stat(min_file_id, empty_file_cnt)` 供 GC / compact 每轮回写；
  - `min_file_id()` / `empty_file_cnt()` 只读访问。
- `FilePageAllocator` 增加同名转发（Pooled 用 `max_fp_id` 近似）。
- 新增 `append_space_size_excludes_empty_files`：10 页 → 10；
  min=1 → 6；min=1 且 1 个空文件 → 2；越界统计饱和为 0。
- 验证：`cargo nextest run -p fkv_mapping` 14 passed。

未做：`update_stat` 尚无生产调用点（依赖 GC / compaction 编排）；
tail 全空时 `max_fp_id` snap-up 行为未实现，需与 GC 清理时序一起定。

## 复审（2026-11-04）

已落地（核对当前源码）：

- `AppendAllocator` 增 `min_file_id: FileId` / `empty_file_cnt: u32`
  （`crate/core/fkv_mapping/src/lib.rs:239-245`）与 `space_size`（`:275`，按文件边界
  扣除空文件、饱和返回 0）、`update_stat`（`:282`）、`min_file_id()`（`:288`）、
  `empty_file_cnt()`（`:292`）。`free` 仍为 no-op。
- `FilePageAllocator` 同名转发 `space_size`（`:380`）、`update_stat`（`:388`）
  （Pooled 分支 `update_stat` no-op 处理）。
- 测试 `fkv_mapping::mapping::append_space_size_excludes_empty_files`
  （10 页 → 10；min=1 → 6；min=1 且 1 空文件 → 2；越界饱和 0）。

## 阻塞

卡点：剩余全部工作都需要 store 级的 `pages_per_file_shift` / `segments_per_file_shift`
并落在本 lane **禁改的 `crate/engine/fkv_store/src/lib.rs`**：

- 由落盘 mapping 反推 `min_file_id`/`empty_file_cnt`（对齐 C++ `Replayer`）必须知道
  shift，而 `PageMapper::with_tbl`（`fkv_mapping/src/lib.rs`）与
  `fkv_root::ReplayArgs`（`crate/core/fkv_root/src/lib.rs:11-24`）都不持有 shift；
  接线的自然位置是 `fkv_store` 的 reopen / `truncate`（`fkv_store/src/lib.rs:1294-1340`）。
- tail snap-up（空尾时把 `max_fp_id` 对齐到新文件边界）与 `truncate`/GC 清理时序绑定，
  需 `fkv_store::maintain`（`fkv_store/src/lib.rs:2160-2170`）配合，同样禁改。
- 唯一 O(1) 收益点 `compact_if_needed` 空盘快路径（`fkv_store/src/lib.rs:2143-2170`）
  也在禁改文件内。
- `MappingTbl::file_stat`/`PageMapper::update_stat_by_scan` 即使在 `fkv_mapping`
  内可写，也无任何 lane 内调用方（`space_size` 的消费者在 `fkv_store`），
  单独落地即死代码，违反 rust_review「零死代码」，故不硬做。
- 反推的 `empty_file_cnt` 精确语义（C++ `replayer.cpp:358-390` 如何计数空文件）
  仓内不可核验（`cpp/eloqstore` 只有 build/shim，无 include/src），须由持有 C++ oracle
  的一方确认后再定稿。

已试命令：`grep`/`sed` 定位 `space_size`/`update_stat`/`min_file_id`/`empty_file_cnt`/
`with_tbl` 全部出现点，并核实 `fkv_root::ReplayArgs`、`fkv_store` 接线点（未改动任何文件）。

需要什么：`crate/engine/fkv_store/src/lib.rs` 的 owner（或解锁该文件的会话）完成
阶段 2 接线：`truncate` 后 `wipe_tail`、`compact_if_needed` 空盘 O(1) 快路径、
`maintain` 中 GC 后回写统计、reopen 时按 shift 反推统计；并结合 C++ oracle 确认反推语义。
`fkv_mapping` 侧的 `file_stat`/`wipe_tail` 可由该会话或本 lane 解锁后一并补上。

## 进展（2026-11-04，Lane E 续做：fkv_mapping 侧派生）

本轮补齐审查结论「阶段 1」中 lane 内可做的反推部分：

- `MappingTbl::file_stat(pages_per_file_shift) -> (Option<FileId>, u32, Option<FilePageId>)`
  （`crate/core/fkv_mapping/src/lib.rs`）：由活跃槽位反推「最老非空文件 / 区间内空文件数 /
  最大 live fp id」，对齐 C++ `Replayer` 从 `using_fp_ids` 反推。两遍扫描换 `O(文件数)`
  内存，不为大 mapping 复制整表。
- `FilePageAllocator::min_file_id()` 转发（Pooled 返回 0）。
- `PageMapper::update_stat_by_scan(shift)`：扫当前映射并 `update_stat` 回写；
  派生值回退时整体忽略（不用 `debug_assert`，避免 release panic 与 min/empty 计数错配）。
- `PageMapper::space_size(shift)` / `min_file_id()`：只读访问器，供上层快路径与测试。
- 测试：`fkv_mapping::mapping::file_stat_derives_min_holes_and_max_fp`、
  `fkv_mapping::mapping::update_stat_by_scan_sets_space_size`
  （空文件区间 → `space_size == 含活跃页文件数 × 每文件页数`；空表保持 0）。
- 验证：`cargo nextest run -p fkv_mapping -p fkv_buf` → 21 passed；
  clippy `-p fkv_mapping --all-targets -- -D warnings` 零告警；`cargo check -p fkv_root -p fkv_large` 通过。

仍未做（承接上方 `## 阻塞`）：空尾 `max_fp_id` snap-up（`wipe_tail`）、`fkv_store` 接线
（reopen 调 `update_stat_by_scan`、`truncate` 后 snap、`compact_if_needed` 空盘 O(1) 快路径、
`maintain` GC 后回写）。这些都需要 store 级 shift 与禁改文件 `fkv_store/src/lib.rs`，
故本条保持 `issue/todo/` 阻塞态，未移 done。

## DONE（2026-11-04）

核对当前源码，统计、派生、snap-up、store 接线全部落地，无需再改代码。

### 证据

- 统计字段与公式：`crate/core/fkv_mapping/src/lib.rs:369-371`（`min_file_id`/`empty_file_cnt`）、
  `:402` `space_size`、`:409` `update_stat`、`:429` `min_file_id`、`:433` `empty_file_cnt`。
- 空尾 snap-up：`:418` `skip_to_next_file`（`space_size == 0` 幂等，推进到下一文件边界并回写
  `min_file_id`，对齐 C++ `DoCompactDataFile` 空 `fp_ids` 分支）。
- 转发层：`FilePageAllocator` `:522/:530/:536/:543`；`PageMapper` `:841` `space_size`、
  `:850` `update_stat_by_scan`、`:861` `skip_empty_space`、`:865` `min_file_id`。
- 由映射反推（对齐 C++ `Replayer`）：`MappingTbl::file_stat`
  `crate/core/fkv_mapping/src/lib.rs:313`（两遍扫描，活跃页 → 最老非空文件/空文件数/最大 live fp）。
- store 接线：
  - reopen：`crate/engine/fkv_store/src/lib.rs:1683` `install_replay_state`，
    `:1709` 调 `update_stat_by_scan`。
  - compact 后回写：`:2184` `compact_file_li`，`:2217` 调 `update_stat_by_scan`。
  - O(1) 快路径：`:2853` `needs_maintain` 用 `space_size`（`:2866`），`:2879`
    `compact_if_needed` 以 `needs_maintain` 为门；空 mapping 时 `:2900` `skip_empty_space`。
  - GC 后同步：`:2922` `maintain` → `gc` → `:3002` `refresh_space_stat`
    （空表 `skip_empty_space` `:3010`，否则 `update_stat_by_scan` `:3012`）。
- 测试：
  - `fkv_mapping::mapping::append_space_size_excludes_empty_files`（`tests/mapping.rs:145`）
  - `fkv_mapping::mapping::skip_to_next_file_zeroes_space`（`:68`，snap-up 幂等/边界）
  - `fkv_mapping::mapping::file_stat_derives_min_holes_and_max_fp`（`:290`）
  - `fkv_mapping::mapping::update_stat_by_scan_sets_space_size`（`:305`）
  - `fkv_store::gc_store::empty_mapping_stops_repeated_maintenance`（`tests/gc_store.rs:150`）
  - `fkv_store::gc_store::maintain_clears_amplification_trigger`（`:189`）

### truncate 不直接 snap 的说明

复审计划阶段 2 曾建议 `truncate` 内调 `wipe_tail`，当前实现有意不做：`truncate` 后保留
`space_size > 0` 使 `needs_maintain` 为真（`gc_store.rs:150` 锁定），由 `maintain` 的
`gc` 删盘后 `refresh_space_stat` 再 snap。若在 `truncate` 内提前 snap，`space_size` 归零会
让死文件失去回收触发点。语义仍与 C++ 一致（snap 发生在空间回收完成之后），且 rollback 由
`BatchJournal` 克隆 allocator 保证。

### 验证

- `cargo check -p fkv_mapping -p fkv_store -p fkv_gc`：通过。
- `cargo nextest run -p fkv_mapping -p fkv_store -p fkv_gc -p fkv_runtime --no-fail-fast`：
  169 passed，1 skipped。
- `cargo clippy -p fkv_mapping -p fkv_store -p fkv_gc --no-deps --all-features --all-targets -- -D warnings`：零告警。

## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:367-371`（min_file_id/empty_file_cnt）、`:400` space_size、`:407` update_stat、`:417` skip_to_next_file（ceil 语义，见 core-26）、`:311` MappingTbl::file_stat、`:893-905` update_stat_by_scan（空表走尾文件分支）；store 接线：`fkv_store/src/lib.rs:1787`（install_replay_state 扫描回写）、`:2978-2991`（needs_maintain 用 space_size）、`:3247-3267`（refresh_space_stat）。测试 `tests/mapping.rs:71/169/314/329` + `fkv_store/tests/gc_store.rs:151/236`。行号相对 DONE 小节有 ±3 漂移（store 热文件），语义全部在位。
