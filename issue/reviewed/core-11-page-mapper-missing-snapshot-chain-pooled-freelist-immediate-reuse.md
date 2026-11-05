# core-11 PageMapper 无 MappingSnapshot 代际链，Pooled 模式物理页立即回池可被 in-flight 读复用

## 位置
- Rust: `crate/core/fkv_mapping/src/lib.rs:334`（`PageMapper` 结构：只有 `tbl`/`allocator`/`dirty_li`/`garbage_file_page_li`/`journal`，无任何快照引用）、`:456`（`remap_page`：`self.allocator.free(old_file_page_id)` 立即回池）、`:262`（`PooledFilePages::free`：`free_li.push`）、`:253`（`PooledFilePages::allocate`：`free_li.pop` 立即复用）
- C++: `eloqstore/include/storage/page_mapper.h:160-174`（`to_free_file_pages_` + `next_snapshot_`：待回收物理页必须等前序快照释放）、`eloqstore/src/storage/page_manager.cpp:731-764`（`FreeMappingSnapshot`：快照引用清零后 `pool->Free(std::move(to_free_file_pages_))`）、`eloqstore/src/storage/page_manager.cpp:405-410`（`UpdateRoot`：非 append 模式 `prev_snapshot->next_snapshot_ = new`，强制快照释放顺序）

## 现状
Rust `PageMapper` 直接持有可变 `MappingTbl` 与分配器。`remap_page`/`free_page` 把旧 `FilePageId` 立刻交回 `FilePageAllocator`：
- `Append` 分支 `free` 为 no-op（`:227`），当前 `fkv_store` 全部用 Append（`crate/engine/fkv_store/src/lib.rs:857,1258`），所以现状安全；
- `Pooled` 分支立即 `free_li.push`，下一次 `allocate` 即可能弹出同一物理页。

没有任何「该物理页是否仍被 in-flight 读者引用」的概念：无快照引用计数、无 retired 列表、无 `to_free_file_pages_` 等价物。

## C++ 实现
C++ 写路径 `MakeCowRoot` 深拷贝 `PageMapper`，读者持有 `MappingSnapshot::Ref`；写路径 `AddFreeFilePage` 只把物理页记入当前快照的 `to_free_file_pages_`，`UpdateRoot` 用 `next_snapshot_` 链强制「前一个快照释放后才轮到下一个」，物理页要等到没有任何快照引用它才回 `PooledFilePages` 池（`FreeMappingSnapshot`）。这是 append 之外模式下读写互不干扰的核心保护。

## 差异与风险
- 当前安全完全依赖两个巧合：shard 串行处理请求（`crate/engine/fkv_shard/src/lib.rs:177-236` `while rx.recv().await { handle().await }` 无任务交错）+ store 硬编码 `AllocMode::Append`。`Pooled` 只在 core 测试出现。
- 一旦满足以下任一条件即变成 P0 级损坏：① shard 引入 in-flight 并发（读任务 await IO 期间写任务 remap/free）；② 任何上层启用 `Pooled`（branch/standby 场景 C++ 正是 Pooled 主用户）。读者拿旧 `page_id` 查 mapping 得到已重分配的物理页，读到另一 key 的新数据。
- `garbage_file_page_li` 只是「通知 GC 哪些页死了」，不承担回收时序保护。
- `ValType::PageId` 空闲链语义（C++ 快照内自由表）在 fkv 也不存在，但与 core-02 交叠，此处不重复。

## 对齐建议
- 短期（保守护栏）：`PooledFilePages::free` 处加 `debug_assert!` 与文档注释，声明「仅在 shard 串行模型下允许立即复用；接入并发前必须引入 retire 机制」；在 `fkv_store` 入口断言 mode == Append。
- 中期（真并发前）：`PageMapper` 增加批次 retire 列表——写批次 commit 时把本批旧物理页挂入 `retired` 队列，待所有 in-flight 读（generation 计数）落定后批量回池；即 C++ `next_snapshot_` 链的轻量替代。

## 性能优化建议
- retire 队列按批次 `Vec` 批量挂接/清空，避免逐页 `Option` 判断；Append 模式下该列表恒为空，零成本。

## 严重度
P1（当前配置下行为等价；设计保护缺失，并发/Pooled 一旦引入即升 P0）

## 审查结论（2026-11-04）

**总结论：部分采纳。** 诊断成立，当前线上配置（Store 硬编码 `AllocMode::Append` + shard 单线程串行）下无实际故障；但核验发现比 issue 描述更具体的一处确定性缺陷：`remap_page` 在 Pooled 下**先 free 后 allocate**，必然拿回同一个 `FilePageId`，等于「COW 原地覆盖」。这不只影响并发读，还会在崩溃窗口让旧 manifest 指回被覆盖的物理页。建议按下方计划修 `remap_page` 顺序并补文档不变量；完整快照链/retire 队列推迟到真正启用 Pooled 时单独立项。

### 逐条核验

| 论断 | 证据（文件:行） | 判定 |
|---|---|---|
| `PageMapper` 无任何快照引用 | `crate/core/fkv_mapping/src/lib.rs:334-342`：仅 `tbl`/`allocator`/`dirty_li`/`garbage_file_page_li`/`changed_page_li`/`journal`；全 crate 无 `MappingSnapshot` 结构（`:3` 只是注释提及） | 属实 |
| `remap_page` 立即回池 | `crate/core/fkv_mapping/src/lib.rs:456-468`：`:462` `allocator.free(old)`，`:465` `allocator.allocate()` | 属实，且顺序颠倒：Pooled 下 `free_li.pop()` 恰好取回刚 push 的旧页，`new_fp == old_fp` |
| `PooledFilePages` 立即复用 | `:253-259` `free_li.pop()`；`:262-264` `free_li.push()` | 属实 |
| `AppendAllocator::free` 为 no-op | `:227` | 属实 |
| Store 全部用 Append | `crate/engine/fkv_store/src/lib.rs:836,890,1263,1281,1671,1691,1816,1893` 全部 `AllocMode::Append`；`State` 持 `&mut self`、无锁无 `Arc` | 属实（issue 所引 `:857,1258` 为旧行号，已漂移） |
| 安全依赖 shard 串行 | `crate/engine/fkv_shard/src/lib.rs:103-105` 每 shard 单 `thread::spawn`；`:177` `while let Ok(envelope) = rx.recv().await { … handle(&mut state, req).await … }`，无 `spawn`/`join`/`select!` 交错 | 属实 |
| `Pooled` 只在 core 测试出现 | `crate/core/fkv_mapping/tests/mapping.rs:36,45`、`crate/core/fkv_large/tests/managed.rs:47`、`tests/large.rs:166,206` | 属实 |
| `garbage_file_page_li` 用于通知 GC | 唯一生产消费点 `crate/engine/fkv_store/src/lib.rs:1314` `let _ = mapper.take_garbage_file_page_li();` 直接丢弃，无 GC/compact 消费 | **部分失实**：目前是只写不读的死数据，不承担任何语义 |
| C++ `to_free_file_pages_` + `next_snapshot_` 链 | `eloqstore/include/storage/page_mapper.h:160-174`；`eloqstore/src/storage/page_manager.cpp:402-411`（`UpdateRoot` 串链）、`:731-756`（`FreeMappingSnapshot` 引用清零后 `pool->Free`） | 属实 |
| 严重度 P1 | 当前 Append+串行下行为等价；Pooled 一旦启用即为持久化/并发双料损坏，应视作 P0 前置缺陷 | 采纳，措辞可保留 P1（「当前配置」） |

补充：C++ 的 COW 语义是「先分配新物理页，旧页只登记进 `to_free_file_pages_`」，Rust `remap_page` 恰好相反。即使完全不考虑并发，Pooled 下原地覆盖也会在「写新页 → 未更新 manifest 崩溃」窗口内破坏旧 root 指向的数据。

### 精确开发计划（采纳部分）

1. **修 `remap_page` 顺序**（`crate/core/fkv_mapping/src/lib.rs:456-468`）
   - 先 `let file_page_id = self.allocator.allocate();`，再 `self.allocator.free(old_file_page_id);`，最后 `tbl.set`。
   - `journal_change`/`changed_page_li`/`garbage_file_page_li` 语义不变；`abort_batch` 靠整份 `FilePageAllocator` 快照回滚，仍正确。
   - 验收：新增 `fkv_mapping` 测试——Pooled 下先 `allocate_page` 再 `remap_page`，断言新 fp ≠ 旧 fp、旧 fp 进入 `free_cnt()`；Append 下编号行为不变。
2. **写清不变量，不加会误伤的断言**
   - 在 `AllocMode::Pooled` 与 `PooledFilePages` 顶部加中文文档：仅允许「无在途读者」的单写者模型；启用前必须实现快照代际链或 retire 队列。
   - 不建议按 issue 原提案在 `PooledFilePages::free` 加 `debug_assert!`：`tests/mapping.rs:36-42`、`tests/large.rs:205-209` 明确断言立即复用，会直接打断测试。
3. **收敛 Store 侧构造点（低成本守卫）**
   - 把 8 处 `PageMapper::new/with_tbl(AllocMode::Append, …)` 收敛到一个 `fn table_mapper(...)`，在其中 `debug_assert!(matches!(mode, AllocMode::Append))` 或直接不暴露 mode 参数，防止后续误改。
4. **后续独立 issue（不在本单做）**
   - 启用 Pooled 前需同时引入：`MappingSnapshot` 引用计数、`to_free_file_pages_`、`next_snapshot_` 链（或 generation retire 队列），并改造读路径持有快照。当前 `fkv_task_read` 直接借用 `&PageMapper`，没有快照语义，单加 retire 列表解决不了。
5. 改完跑 `./sh/clippy.sh` 与 `./test.sh`。

### 风险

- 调整 `remap_page` 顺序会改变 Pooled 分配序列（仅测试可见）；Append 编号不变，线上无影响。
- 仅补文档/断言不能消除并发风险；若后续有人把 Store 改成 Pooled 而跳过步骤 4，仍会损坏，需要 review 门禁。
- 步骤 4 属设计级改造，跨 `fkv_mapping`/`fkv_task_read`/`fkv_store`，不应塞进本 issue。

### 附带修正

- issue 中 `:857,1258` 已漂移为 `:836,890,1263` 等；`garbage_file_page_li` 目前无消费者，`## 差异与风险` 第 3 条应改为「无回收时序保护，且当前无人消费」。

## 进展（2026-11-04，部分完成）

- `PageMapper::remap_page` 改为「先 allocate、再 free 旧页」。
  Pooled 的 free list 是 LIFO，旧顺序会立刻取回刚释放的同一物理页，
  使 COW 退化为原地覆盖。
- 新增 `remap_does_not_reuse_released_page`：Pooled 下释放 fp0 后 remap
  必须拿到 fp0，且不能拿回当前页的旧 fp1。
- 验证：`cargo nextest run -p fkv_mapping -p fkv_store` 81 passed。

未做：MappingSnapshot 代际链 / retire 队列 / 活跃快照并入 GC retained set，
属真并发前置，继续跟踪。

## DONE（2026-11-04）

- `remap_page` 改为「先 `allocate` 再 `free(old)`」，消除 Pooled 下 LIFO
  立即取回刚释放物理页、COW 退化为原地覆盖的确定性缺陷；Append 模式编号不变。
- `AllocMode` / `PooledFilePages` 补中文不变量文档：仅单写者 + 无在途读者可启用
  Pooled，并发前必须实现 C++ `to_free_file_pages_` + `next_snapshot_` 等价机制。
- `fkv_store` 已有编译期守卫：`STORE_ALLOC_MODE` 常量 + `const _` 断言，
  改 Pooled 直接编译失败（优于原计划的运行期 `debug_assert`）。
- 测试：新增 `remap_does_not_reuse_released_page`（Pooled 下 remap 必须拿到
  已释放 fp0，且不能拿回当前页旧 fp1）。
- 验证：`cargo check -p fkv_mapping`、`cargo nextest run -p fkv_mapping -p fkv_store`
  98 passed、`cargo clippy -p fkv_mapping --all-targets -- -D warnings` 零警告。
- 遗留（独立 issue）：`MappingSnapshot` 引用计数 / retire 队列 / 读路径持快照，
  属真正启用 Pooled 或并发读的前置设计，不在本单范围。

## 复审（2026-11-05 r6）

通过。`remap_page` 先 allocate 再 free（`crate/core/fkv_mapping/src/lib.rs:814-820`，注释明示 LIFO 原因）；`fkv_store/src/lib.rs:72-76` `STORE_ALLOC_MODE` + `const _` 编译期断言（改 Pooled 编译失败）；测试 `tests/mapping.rs:155-165 remap_does_not_reuse_released_page`（断言 remap 拿回已释放 fp0 且 `assert_ne!(remapped, 1)`）。快照代际链已由 27a 落地（见该文件）。
