# sto-56 fkv_compaction 空壳 + 页重写计划与 C++ 差异（触发判定、批式搬页、segment 压缩缺失）

## 位置
- Rust: `crate/engine/fkv_compaction/src/lib.rs:1`（整个 crate 只有一行 doc，无任何实现）；实际执行：`crate/engine/fkv_task_compact/src/lib.rs:11-30`（`compact_file` 逐页读-改-写）；触发判定：`crate/engine/fkv_store/src/lib.rs:2153-2167`（`needs_maintain`）、`crate/engine/fkv_gc/src/lib.rs:52-77`（`pick_compact_file`）
- C++: 无独立 compaction 模块；核心实现在 `src/tasks/background_write.cpp:165-329`（`BackgroundWrite::DoCompactDataFile`）、`331+`（`DoCompactSegmentFile`）、触发在 `src/tasks/write_task.cpp:880-892、898-941`（`CompactIfNeeded` / `MapperExceedsAmplification`）

## 现状
1. **crate 空壳**：`fkv_compaction` 按 todo.md §3.7 定义「空间放大分析、页重写计划、segment 重写计划」，实际只有一行 doc。计划逻辑散落在 `fkv_task_compact` 的 30 行里，`fkv_gc` 的 `pick_compact_file` 做候选选择。
2. **触发判定不等价**：Rust `needs_maintain` 用 `mapped = mapper.tbl().len()`（mapping 表**槽位总数**，含已释放逻辑页的 INVALID 槽位）与 `space = mapper.max_fp_id()`（分配器 high-water）；C++ `MapperExceedsAmplification` 用 `mapper->MappingCount()`（**活映射数**）与 `AppendAllocator::SpaceSize()`（扣除整文件空隙后的已分配空间，`write_task.cpp:924-925`）。mapping 稀疏（大量删除后）时 Rust 把空闲槽位计入 mapped → 放大率被低估 → 压缩触发滞后；文件被整文件 GC 后 C++ 的 SpaceSize 自动回落，Rust 的 `max_fp_id` 不回落。
3. **重写计划**：Rust 一次 `compact_if_needed` 只挑**一个**放大率最高的文件（且剔除追加尾文件，`fkv_store:2183-2189`），`compact_file` 逐页 `read_page → remap_page → write_page` 串行（`fkv_task_compact:21-28`）；C++ `DoCompactDataFile` 在**一次** compaction 内对 `begin_file_id..end_file_id` 中所有超阈值文件做搬页：批量读（`max_read_pages_batch=128`）、swizzle 内存页直写（`MovingCachedPages`，`background_write.cpp:276-297`）、搬完后 `allocator->UpdateStat(min_file_id, empty_file_cnt)` 把已清空文件的空隙并入 allocator 统计（324 行）。
4. **空 mapping 分支**：C++ 在 `fp_ids.empty()` 时把分配器 high-water 推到下一文件边界（`background_write.cpp:200-211`），避免反复触发；Rust 无对应处理（reopen 后 `tbl().len()` 归零、`max_fp_id` 保持，`needs_maintain` 的 `mapped==0 → space != 0` 分支永远为真）。
5. **segment 压缩**：C++ `segment_file_amplify_factor` 有完整 segment 重写计划（`DoCompactSegmentFile`，331 行起，用 GlobalRegisteredMemory 固定 buffer 批式搬 segment）；Rust `KvOption.segment_file_amplify_factor` 字段存在但引擎完全未消费，`fkv_store` 无 segment compact 路径。
6. **扩容抖动**：C++ 的 `trigger_compact` 信号由写任务产生、后台 handler 统一判定（`write_task.cpp:880-892`），且 compaction 自身 `UpdateMeta(trigger_compact=false)` 防自触发（`background_write.cpp:155-158`）；Rust 在 `maintain_after_write`（`fkv_shard/src/lib.rs:351-361`）直接同步执行，无 pending 去重。

## C++ 实现
见上；C++ 的设计是「写任务只置 pending flag + O(1) 预判，后台任务做计划与重写」，计划/执行都在 `BackgroundWrite`，与数据文件、segment 文件两套参数（`file_amplify_factor` / `segment_file_amplify_factor`）分离。

## 差异与风险
- 空间放大回收节奏不同源：Rust 在「槽位计数」口径下放大率长期低于 C++，写入密集型 workload 下垃圾文件滞留时间更长、磁盘占用偏高（违反验收标准 6 的 RSS/写放大口径时难以解释）；
- 无 segment 压缩：大 value workload（M5 宣称完成）下 segment 文件不可回收，与 C++ `segment_file_amplify_factor` 行为缺失；
- 空 mapping 反复触发 → 每次写后 maintain 都执行一次全空扫描（`needs_maintain` 为 O(1) 但 `compact_if_needed` 里 `file_live_count` 全表扫 + `list_file_id_li` 目录列举），抖动；
- 单文件粒度搬页没有 swizzle 直写优化（Rust 无 swizzle 内存页概念，页总经 IO，性能差距在 compaction 不可见但放大 IO 次数）。

## 对齐建议
- 填充 `fkv_compaction`：`amplify_stats`（MappingCount/SpaceSize 等价口径）、`compact_plan`（一次遍历所有候选文件）、`segment_compact_plan`；
- `PageMapper` 暴露 `mapping_count` 与 allocator `space_size`（扣除整文件空隙），`needs_maintain` 改用 C++ 口径；
- `compact_file` 改为批式：按 `max_read_pages_batch` 聚合读 + 顺序 remap/write，计划一次覆盖多文件；
- 空 mapping 分支：compact 时把 `max_fp_id` 推到下一文件边界并 `UpdateStat` 等价物；
- 接 `segment_file_amplify_factor` 到 `Store::maintain`/`compact_if_needed`，segment 重写计划复用 `fkv_large` 的 segment IO；
- 写任务侧引入 pending flag（shard 注册表已有 `HasPendingCompact` 的等价物）防自触发。

## 性能优化建议
- 搬页批量 IO（`write_page_li` 已存在）；remap 与 write 合并遍历避免二次扫描 mapping。

## 严重度
P1（行为不一致/功能缺失）

## 审查结论（2026-11-04）

判定：**合理，部分事实已过时**。`crate/engine/fkv_compaction` 目录已不存在（32 任务删除），
计划逻辑实际落在 `fkv_gc::compact_plan` 与 `fkv_task_compact`，"填充空壳"建议作废；
其余差异以 `fkv_task_compact` / `fkv_store` / `fkv_mapping` 实际代码为准，逐条核验：

- 已过时：`fkv_compaction` 空壳（目录不存在）；`AppendAllocator::space_size` / `update_stat`
  已存在（core-12）；`file_stat` / `update_stat_by_scan` 已存在。
- 成立且本次修复：`needs_maintain` 用槽位总数 + `max_fp_id`（已改 `MappingTbl::mapping_count`
  与 `PageMapper::space_size`）；一次只挑一个文件（已改 `fkv_gc::compact_plan` 全量计划 +
  `Store::compact_file_li` 单批次多文件）；逐页串行搬页（已改 128 页批量读写 +
  `live_page_li_by_file` 单次扫描）；空 mapping 反复触发（已加 `skip_to_next_file` 跳界）。
- 成立但未实现，转 todo：`segment_file_amplify_factor` 无消费方 → `sto-56b`；
  写任务 pending flag / 自触发去重（`fkv_shard` 并行会话占用）→ `sto-56c`。
- 已实现部分的验收记录：`sto-56a`。

拆分与落盘：本文件移到 `issue/todo/` 后按 `sto-56a` 完成记录收尾；`sto-56b` / `sto-56c`
保留在 `issue/todo/`。

## DONE（2026-11-04）

L1 闭环（触发口径、页重写计划、文件选择、空 mapping）已在 `sto-56a` 实现并验证；
segment 压缩与写任务 pending flag 作为遗留项留在 `sto-56b` / `sto-56c`。

## 复审（2026-11-05 r6）

通过（父条目）。触发口径/页重写计划/空 mapping（sto-56a）、segment 压缩（sto-56b）、
pending flag 改判（sto-56c）均已按当前源码复核，见各自 r6 复审小节。
