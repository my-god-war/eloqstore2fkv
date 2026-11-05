# sto-52 Replayer GetMapper 分配器重建缺失：empty_file_cnt / PooledFilePages / branch 区间过滤 / cloud term bump

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:1253-1296`（`install_replay_state`，1259 行 `PageMapper::with_tbl(state.mapping_tbl, AllocMode::Append, state.max_fp_id)`）
- C++: `src/replayer.cpp:305-430`（`Replayer::GetMapper`）、`src/replayer.cpp:431-503`（`GetSegmentMapper`）

## 现状
Rust reopen/open_branch 一律把恢复的 mapping 包装成 `AllocMode::Append` 的 `PageMapper`：`with_tbl(..., Append, max_fp_id)`，自由逻辑页走 `rebuild_free_page_li` 收集，但分配器从 `max_fp_id` 起纯追加。没有：
- append 模式下 `empty_file_cnt` 的空文件计数（mapping 中 file_id 跳号统计）；
- `data_append_mode=false` 时的 `PooledFilePages` 原地复用分配器；
- Pooled 路径按 `branch_metadata.file_ranges` 过滤（`FileIdInBranch`）的 free page 集合——只把本 branch 区间内的空闲页放回池；
- cloud/standby 模式下 manifest term ≠ 进程 term 时 `max_fp_id` 跳到下一文件边界（防跨 term 复用同一文件 id）。

`segments_per_file_shift` 的 term bump 同样缺失。

## C++ 实现
`Replayer::GetMapper`（`replayer.cpp:305-430`）：
- `data_append_mode` 为真：收集全部 `using_fp_ids`，`std::sort` 后计算 `empty_file_cnt`（375-386 行），构造 `AppendAllocator(opts, min_file_id, max_fp_id, empty_file_cnt)`（367-391 行）——空文件计数参与 `SpaceSize()` 与 GC 放大率判定；
- 为假（C++ 默认）：构造 `PooledFilePages(opts, max_fp_id_, free_ids)`（425 行），free_ids 遍历 `0..max_fp_id` 且 `FileIdInBranch(ranges, DataFileKey(fid), active_branch)` 过滤（408-423 行），显式注释防止子 branch 回收父 branch 文件页；
- cloud 非 Local 且 `expect_term != 0 && manifest_term != expect_term`：`max_fp_id_ = (max_fp_id_ >> shift) + 1 位后回写`（355-362 行）。
`GetSegmentMapper`（431-503 行）segment 侧同样逻辑（464-475 行 term bump）。

## 差异与风险
- **数据损坏方向（branch 并发）**：C++ Pooled 路径严格按 file_ranges 过滤空闲页；Rust 若哪天接 `PooledFilePages`（`crate/core/fkv_mapping/src/lib.rs:254-259` 已实现），不实现区间过滤会把父 branch 的 file page 复用给子 branch 写——静默覆盖父 branch 活数据。当前 Rust 恒 Append 不触发，但分支预留区间语义（D54）与 C++ 不同源。
- **云模式跨 term 冲突**：C++ 靠 term bump 保证 manifest（旧 term）replay 出的分配器不会重用新 term writer 已占用的文件 id；Rust 云 reopen 无此保护，旧 term 分区与新 term writer 并发写同一 cloud bucket 时存在文件 id 碰撞窗口（依赖 term CAS 的即时性兜底，未验证）。
- **empty_file_cnt 缺失**：直接改变 compaction 触发判定（`MapperExceedsAmplification` 用 `SpaceSize()`）与 GC 语义，见 sto-56。
- **默认模式不对称**：C++ `data_append_mode` 默认 false；Rust reopen 恒 append 意味着「C++ 默认模式写的 store 由 Rust 续写」时分配器策略切换为 append（逻辑正确、空间放大不同）。todo.md 的「L1 Local + append」实际是 Rust 单侧恒 append，与 C++ 默认模式不等价，需在基线声明里明示。

## 对齐建议
- `ReplayState` 携带 `data_append_mode`（由 KvOption 注入 Store）与 `expect_term`；`install_replay_state` 按 C++ GetMapper 两条路径重建分配器：
  1. append：排序 using_fp_ids 算 `empty_file_cnt`，`AppendAllocator::new_with_empty_cnt(min_file_id, max_fp_id, empty_file_cnt)`；
  2. pooled：`PooledFilePages` + `FileIdInBranch(branch_metadata.file_ranges, ...)` 过滤；
  3. cloud/standby（`derive_mode != Local`）且 `expect_term != 0 && state.branch_metadata.term != expect_term` 时对 data 与 segment `max_fp_id` 做边界 bump。
- 同步补 `segment` 侧重建（`segment_mapper` 恒 append + term bump）。

## 性能优化建议
- `empty_file_cnt` 可与 `file_live_count`（`fkv_gc`）共用一次 mapping 遍历，避免 reopen 二次 O(n)。

## 严重度
P1（行为不一致/功能缺失）

## 审查结论（2026-11-04）

判定：**拒绝（重复 + 超出 L1 范围）**，剩余项属 pooled 分配器，已由 sto-59 判定不在对齐范围。

逐条核验：

| 条目 | 现状 | 证据 |
| --- | --- | --- |
| `empty_file_cnt` / `min_file_id` 重建 | 已落地 | `PageMapper::update_stat_by_scan`（`fkv_mapping/src/lib.rs`）在 `install_replay_state` 与 GC/compact 后调用；`AppendAllocator::update_stat` / `space_size` 已对齐 C++ `UpdateStat` / `SpaceSize`；测试 `crate/core/fkv_mapping/tests/mapping.rs`（`append_space_size_excludes_empty_files`、`file_stat_derives_min_holes_and_max_fp`、`update_stat_by_scan_sets_space_size`）。原 issue 落点已因 core-12 实现而消失。 |
| data / segment 跨 term bump | 已落地 | core-22a：`install_replay_state` 对 `max_fp_id` 与 `max_segment_fp_id` 分别按 `pages_per_file_shift` / `segment_shift` 调 `next_file_boundary`（`(max >> shift + 1) << shift`），与 C++ `Replayer::GetMapper` / `GetSegmentMapper` 一致；测试 `crate/engine/fkv_store/tests/term_bump.rs`。 |
| segment mapper 重建 | 已落地 | 同一 `STORE_ALLOC_MODE`（Append）建 `segment_mapper`，term bump 同步生效；segment 侧无 `space_size` 消费端，无需 stat。 |
| `data_append_mode=false` 的 `PooledFilePages` 重建 | 不实现 | `COMPAT_BASELINE` 明示「Rust 恒用 AppendAllocator；C++ 默认 pooled 不在对齐范围，互操作验收限 append store（sto-59）」。Rust 侧已有 `STORE_ALLOC_MODE = AllocMode::Append` 与编译期断言，改成 `Pooled` 直接编译失败（`fkv_store/src/lib.rs:64-75`），正是阻止「半实现分配器」的闸门。 |
| branch `file_ranges` 区间过滤空闲页 | 不实现 | 仅在 pooled 路径有意义；append 模式 `free` 为 no-op，物理页永不复用，分支间不存在「回收父 branch 文件页」风险。 |

拒绝理由：

1. pooled 分配器接线需要同时实现 branch 区间过滤、free page 重建、`data_append_mode` 选项贯通与并发读者 retired 队列，属于 L1 扩容，不能只落本 issue 的一条。
2. sto-59 已记录 `data_append_mode` 字段「语义登记、false 不切换分配器」，并明确把 pooled 接线留给后续扩容；本 issue 若单独实现即为重复且会绕过该闸门。
3. core-22a 已覆盖 term bump，core-12 已覆盖 `empty_file_cnt`，本 issue 剩余论断与代码现状不符。
4. `doc/migration.md` §8.2 已将 `GetMapper` 分配器重建列为「恢复语义不等价、不纳入互操作保证」，与 `COMPAT_BASELINE` 一致。

后续：若要升级 L1 到 pooled，请以独立 issue 立项，先补 `MappingSnapshot` 代际链 / retired 队列（见 `core-11`）与 `FileIdInBranch` 过滤，再一次性接 `STORE_ALLOC_MODE`。
