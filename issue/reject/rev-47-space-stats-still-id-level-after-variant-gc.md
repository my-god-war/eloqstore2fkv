# rev-47 P3：rev-39 变体 GC 落地后，file_live_count / 空间统计仍按 file id 计数；其 DONE 遗留段「wrapper 未透传」声称已过期

## 处置（2026-11-05 甄别）

**部分采纳：开发项拒绝（统计维持 id 级 = C++ 同口径，非偏离）；更正记录落地
（已追加到 `issue/reviewed/rev-39-*.md` 遗留段之后）。** 本文件事实部分全部
核实无误（引用行号逐处命中），但「空间放大被低估、维护时机偏晚」的定性漏了
C++ 对照——**C++ 不是近似，是完全同口径**，逐处证据：

- C++ 维护触发 `WriteTask::MapperExceedsAmplification`（eloqstore
  `src/tasks/write_task.cpp:911-941`）用 `alloc->SpaceSize()`（分配器内存视图）
  对 `MappingCount()`——当前 branch 专属视图，死变体物理盘占用同样不可见；
  与 Rust `needs_maintain`（fkv_store:3448-3466）逐条对应（含 `mapping_cnt==0
  && space>0` 分支）。
- C++ compact 候选 `BackgroundWrite::DoCompactDataFile`（`src/tasks/
  background_write.cpp:165-265`）是 `[begin_file_id, end_file_id)` 的纯 id
  空间循环，由当前 mapping 的 fp_ids + 分配器边界驱动，**根本不查盘面**；
  每文件放大率 `pages_per_file / live_cnt` 与 Rust `compact_plan`（fkv_gc:
  47-72）同式。Rust 的 `candidate_li` 来自盘面 id 去重，反而看得略多（死变体
  id live==0 → 交给 GC，不 compact，与 C++ 行为等价）。
- C++ `AppendAllocator::SpaceSize/UpdateStat`（`src/storage/page_mapper.cpp:
  665-693`，`min_file_id/empty_file_cnt` 由当前 mapping 反推）与 Rust
  `space_size/update_stat_by_scan`（fkv_mapping:880-905）同构。
- 回收窗口同构：两侧维护钩子（C++ `Compact()` 末尾 `RunFileGc`；Rust
  `maintain` :3663-3669 = compact → file_gc → refresh_space_stat）都在同一轮
  把死变体交给 GC——rev-39 后两侧 GC 均为变体精确判删，统计盲区同样被一轮
  维护周期界定。

无用户可见影响：`space_size` / `file_live_count` 仅为内部调度启发（`needs_
maintain` / `compact_plan` 候选），fkv_metrics 全部 gauge 中唯一空间项是
`cache_bytes`（云 overlay 本地 cache，对应 C++ `local_space_used`），与 per-file
空间统计无关；C++ `file_gc.cpp` 亦无任何统计暴露（grep `stat` 仅 readdir+stat）。

因此：修法第一段（统计/候选升变体口径）**会制造对 C++ 的新偏离**而非消除偏离；
第二段的 COMPAT_BASELINE 登记路径同样不适用——baseline 只登记真实的有意偏离，
此处 Rust 与 C++ 同口径，登记即失实。开发项关闭。更正记录（wrapper 已透传）
属实且有价值，已直接追加到 rev-39 归档件遗留段之后，防后续会话按过期声称
重复排查。

来源：r7 第 3 组复审（`issue/reviewed/00-r7-group3.md`）。rev-39（变体粒度 GC）
复审通过归档，其 DONE 自认两条 lane 外遗留中：统计侧无条目承接（原开发计划
第 3 项被裁出 lane，DONE 明言「另行立案」，不立即立项即失联——eng-93 同款
模式）；wrapper 侧经亲读已被后续改动消化，但遗留段原文未更正，会误导后续
会话按「生产 GC 泄露修复不生效」重复排查。本条承接前者、更正后者。

## 事实（2026-11-05 r7 亲读）

1. **统计 / compact 候选均 id 级**：
   - `fkv_gc/src/lib.rs:12-21` `file_live_count`：一遍 mapping 按
     `decode_id(value) >> shift` 统计每 **file id** 的 live page 数——mapping
     是 id 级逻辑视图，同 id 的跨 branch 变体在统计中不区分。
   - `fkv_store/src/lib.rs:3388-3389` `compact_before_branch`：`live_map` +
     `list_file_id_li`（id 级）喂 `compact_plan`，死变体文件既不单列放大率、
     也不参与搬空决策（同 id 有 live page 即整 id 视为活）。
   - `fkv_store/src/lib.rs:3731-3756` `refresh_space_stat`：来自 mapper 自身
     `update_stat_by_scan` / `skip_empty_space`（分配器视角，id 级），
     死变体的物理盘占用在 `needs_maintain` / 空间放大口径中不可见。
   - 后果（与 rev-39 原意见一致的方向）：多 branch / 多 term 下死变体已能被
     GC 变体判删回收（rev-39 已修），但**统计与 compact 触发口径**仍看不见
     它们——空间放大被低估、维护时机偏晚。属观测 / 资源治理精度，非正确性。
2. **wrapper 未透传的声称已过期（更正记录，无需开发）**：rev-39 DONE 遗留段
   写「`fkv_metrics` / `fkv_shard` wrapper 未透传变体接口，经其 GC 的泄露修复
   不生效」。当前工作区亲读：`fkv_metrics/src/lib.rs:784-786`（list）与
   `:845-854`（remove）、`fkv_shard/src/backend.rs:447-451/:535-545`（Local/
   Mem/Cloud 三后端转发）、`fkv_shard/src/roots.rs:289-290/:333-342` 均已透传，
   且 `fkv_metrics/tests/metered_forward.rs:128` `list_file_variants_keeps_
   identity` 在位（文件 mtime 04:03-04:07，早于本轮复审，非在途）。生产 GC
   路径（`Store::gc` → `GcScope::list/remove_*_variants`，fkv_task_gc:714-777）
   经 wrapper 后仍为变体粒度，泄露修复已生效。

## 修法（P3，资源治理精度）

- `file_live_count` / `compact_plan` 候选升级变体口径的可行性评估：mapping
  仅含当前 branch 视图的 id → 页指向（不含 (branch, term)），变体精度需要结合
  `list_file_variants` 的盘面三元组与 per-manifest 归属（rev-39 的
  `RefKnowledge` 已有现成归属逻辑，fkv_task_gc 可复用）；或维持 id 级、仅在
  空间统计侧按 `list_file_variants` 的变体数加权死变体占用。
- 若 owner 裁定维持现状（id 级统计 + GC 兜回收），则按 eng-93 先例在
  `COMPAT_BASELINE` 或 doc 登记该统计口径偏离，关闭本条。

## 验收（如选择开发）

- 多 branch 同 id 场景：死变体的物理字节数计入该表空间统计 / 放大率分母；
- `fkv_gc` / `fkv_store` 相关测试（`tests/gc.rs` 变体用例扩展统计断言）；
- 若裁定登记：登记行进 `COMPAT_BASELINE` 并同轮重跑 `gen_fixtures.sh`
  （eng-93 先例，`tool/fkv_ledger/src/fixture.rs:342` flatten 机制）。

## 甄别（2026-11-05 r7 立项）

- 去重：`issue/new/` + `issue/todo/` grep `file_live_count|list_file_variants|
  remove_file_variant` 0 命中；rev-39（reviewed）遗留段是出处记录，归档后无
  执行载体。eng-90（merge clone）、rev-44（注册内存 slab）、rev-45（快照克隆）
  均不同物。
- P3 统计精度 + 一条事实更正，无代码紧急性；可与任意 lane 并行。
