# eng-89 P1：compact / compact_segment 落 manifest 前缺 data fsync，崩溃窗口可致存活页永久损坏

热点文件：需与 owner 会话协调（`fkv_store/src/lib.rs` 正被并行会话热改，本轮复审期间行号多次漂移）。

来源：r6 store+write lane 复审（对比 C++ `UpdateMeta` 提交时序时发现）。

## 证据（2026-11-05 亲读，行号以当日为准，漂移 ±20 以内容定位）

`crate/engine/fkv_store/src/lib.rs` 全文仅 2 处 `io.sync_data`：

- put 路径 `:1497`：`put_at_inner` 内 `self.io.sync_data(tbl).await?`，注释明言
  「先让数据页落盘，再写 manifest（对齐 C++ WaitWrite → SyncData → FlushManifest）」；
- clean_expired 路径 `:2226-2227`：`sync_data` 之后才 `append_mapping_log`。

而两条 compact 路径**均无 data fsync**，搬迁页写完立即追加 manifest：

```rust
// compact_file_li（:2345 起；:2384）
let log_result = self.append_mapping_log(tbl, root_id, ttl_root_id).await;

// compact_segment_file_li（:3167 起；:3207）
let log_result = self.append_mapping_log(tbl, root_id, ttl_root_id).await;
```

`append_mapping_log` 内部只对 **manifest 文件**做 append + fsync；搬迁页经
`write_page_li` / `write_segment` 落盘后仅记入 `dirty` 集合
（`fkv_io_compio/src/lib.rs:2035/2110/2409`），等**下一次** put 的 `sync_data` 才落盘。

放大因素：`maintain`（:3219 起）在 `compact_if_needed → compact_segment_if_needed` 之后
紧接着 `gc` 删除搬空的旧文件——旧文件里可能存有唯一已落盘副本，新页仍在 page cache。

## C++ 对照（eloqstore 82b774c，亲读）

- `src/tasks/write_task.cpp:854-868` `WriteTask::UpdateMeta`：
  `WaitWrite() → IoMgr()->SyncData(tbl_ident_) → WaitPendingUploads() → FlushManifest()`，
  **数据 fsync 在 manifest 之前**。
- `src/tasks/background_write.cpp:146-157` `BackgroundWrite::Compact`：
  `DoCompactDataFile / DoCompactSegmentFile` 之后统一走
  `UpdateMeta(/*trigger_compact=*/false)`，即 compact 同样先 `SyncData` 再 `FlushManifest`。

## 差异与风险

崩溃窗口：compact 追加 manifest（fsync 成功，新 fp 映射已持久）→ 搬迁页尚未 fsync →
进程崩溃 / 掉电 → 重启后 manifest 指向的新 fp 内容为垃圾 / 零页 → 存活逻辑页 checksum
失败（`Corrupted`），且旧文件可能已被 `gc` 删除，**不可恢复**。这不是「丢已确认写」
（compact 本身无用户 ack），而是把原本完好的存活数据变坏，严重度高于丢写。

在 macOS（无 O_DIRECT）与 `fixed_buffer_size=0`（默认，见
`fkv_runtime/tests/write_group_commit.rs:41` 的 `RuntimeConfig`）配置下数据写全部走
page cache，窗口真实存在。

## 建议修法

在 `compact_file_li` 与 `compact_segment_file_li` 的 `append_mapping_log` 之前各加
`self.io.sync_data(tbl).await?;`（失败走既有 abort 分支，与 put 路径同构；dirty 集合
机制已保证覆盖 data / segment 两类文件）。`compact_before_branch` 与 `maintain` 复用
这两个入口，无需另改。

## 验收

- failpoint `AppendManifest:Write@1!err=IoFail` 前先断言 `SyncData` 已被调用过
  （如经 `MeteredIo` 的 fsync 计数或注入 `SyncData` failpoint 使 compact 在
  「已搬迁、未 sync」处失败，断言回滚后旧 mapping 仍可读）。
- 新增测试：compact 搬页后、manifest 追加前注入 `SyncData` 失败 → abort 回滚
  `max_fp_id` / mapping（复用 `tests/maintenance_batch.rs::compact_manifest_failure_rolls_back_mapping`
  骨架，补 sync 调用断言）。
- `cargo nextest run -p fkv_store`；clippy 零警告。

## 依赖 / 重叠

- core-25（done）只修了 mapper journal 的 commit/abort 顺序，不涉及 fsync 时序；
  其测试 `compact_manifest_failure_rolls_back_mapping` 注入点是 manifest 写失败，
  恰好测不到「数据未 sync」窗口。
- rev-35（new，另一 lane）是 failpoint 注入点缺失，与本条正交；本条验收可借用其
  建议的 `SyncFiles` 注入点思路，但不依赖。

## 甄别（2026-11-05 r6）

判定：**采纳**。全部证据逐条亲读复核成立（行号为当日实读，仍可能漂移）：

1. Rust 缺失证据成立：`fkv_store/src/lib.rs` 全文确仅 2 处 `io.sync_data` ——
   `:1497`（`put_at_inner`，`:1495-1496` 注释明言对齐 C++
   WaitWrite → SyncData → FlushManifest，随后 `:1515` 才 `append_mapping_log`）与
   `:2226-2227`（`clean_expired`，sync 在 manifest 之前）。`compact_file_li`
   （`:2345`）在 `:2384`、`compact_segment_file_li`（`:3167`）在 `:3207` 直接
   `append_mapping_log`，两处均无 data sync。
2. 「写后仅记 dirty」成立：compio 后端 `write_page_li`（`fkv_io_compio/src/lib.rs:2035`，
   `:2106` 仅 `mark_dirty`）、`write_segment`（`:2370`，`:2405` 仅 `mark_dirty`）；
   compact 搬迁确实经此入口（`fkv_task_compact/src/lib.rs:94`）。`sync_data`
   （`:2114`）才对 dirty 集合批量 fdatasync（data + segment 两类文件都覆盖），
   且 `:2115` 已有 `fail_point!("SyncData")` 可直接用于测试。
3. 放大因素成立：`maintain`（`:3224-3231`）与 `compact_before_branch`
   （`:2955-2992`）都是 compact 后紧跟 `gc`；gc 的 retained 集合取自新 manifest +
   当前 mapping（`:3329-3365`），搬空旧文件会被删，而新页仅剩 page cache 副本。
4. C++ 对照成立：`write_task.cpp:854-868` `UpdateMeta` 严格
   WaitWrite → SyncData → WaitPendingUploads → FlushManifest；
   `background_write.cpp:157` compact 后统一走 `UpdateMeta(false)`，`:160` 才
   RunFileGc（C++ 连 GC 都在 SyncData 之后）。Rust 的 put/clean_expired 也已有
   同序，唯独两条 compact 路径缺失，属真实偏离而非有意设计。
5. 崩溃语义判断正确：manifest 先于数据持久 → 新 fp 指向垃圾/零页 → checksum
   `Corrupted` 且旧文件可能已删，属「把好数据变坏」，P1 定级合理。

与相邻 issue 的边界（不重复部分）：

- core-25（实际在 `issue/reviewed/`，含 DONE+复审记录）只修 commit/abort 顺序
  （进程内回滚语义），其测试 `maintenance_batch.rs:22`
  `compact_manifest_failure_rolls_back_mapping` 注入的是 manifest 写失败，
  全文件无任何 SyncData 注入——恰好测不到本条的「数据未 sync」崩溃窗口。二者正交，
  本条不与 core-25 重复。
- rev-35（`issue/new/`）是注入点缺失（OpenOrCreateFD:CreateFile / SyncFiles），
  与本条互补：本条修完后所需的 `SyncData` failpoint 已存在，不依赖 rev-35。
- core-23（gc 并发防护）、io-21（无 O_DIRECT）均不涉及此时序问题；r6 lane 索引
  `issue/reviewed/00-r6-store-write.md:48-51` 已把本条列为新立项 P1，本文件即唯一载体。

开发计划（**fkv_store/src/lib.rs 是并行会话热点文件，动手前需与 owner 会话协调**）：

- 改 `crate/engine/fkv_store/src/lib.rs` 两处，与 `clean_expired` `:2225-2228` 同构：
  - `compact_file_li`：把 `:2384` 改为
    `let log_result = async { self.io.sync_data(tbl).await?; self.append_mapping_log(tbl, root_id, ttl_root_id).await }.await;`
  - `compact_segment_file_li`：`:3207` 同改。
  失败（含 sync 失败）继续走既有 `settle_mapper_batch(tbl, log_result.is_ok())`
  abort 分支；`compact_before_branch` / `maintain` / `compact_if_needed` 均复用
  这两个入口，无需另改。dirty 集合机制已覆盖 data + segment，一次 `sync_data` 即够。
- 测试（`crate/engine/fkv_store/tests/maintenance_batch.rs`）：
  - 复用 `compact_manifest_failure_rolls_back_mapping` 骨架新增一条：compact 搬页后、
    manifest 前注入 `SyncData` failpoint（`SyncData!err=IoFail`）→ 断言 abort 回滚
    `max_fp_id` / mapping / dirty_page_li 与调用前一致、旧 mapping 可读可重试；
  - 正向断言：compact 成功路径 `SyncData` 恰好被调用一次（failpoint 计数或
    MeteredIo fsync 计数），证明 manifest 追加前数据已落盘。
  - `cargo nextest run -p fkv_store`；clippy 零警告。

## DONE（2026-11-05）

改动：

- `crate/engine/fkv_store/src/lib.rs`
  - `compact_file_li`：把 `append_mapping_log` 改为
    `sync_data(tbl) → append_mapping_log(tbl, ...)` 的组合，与 `put_at_inner` /
    `clean_expired` 同序；`sync_data` 失败走既有 `settle_mapper_batch(tbl, false)` abort。
  - `compact_segment_file_li`：同样先 `sync_data` 再落 manifest。两条入口的复用方
    （`compact_if_needed` / `compact_segment_if_needed` / `compact_before_branch` /
    `maintain`）自动继承，无需另改。
- `crate/io/fkv_io_compio/src/lib.rs`
  - 新增测试用计数 `data_sync_num`（`sync_data` 入口命中次数）与只读访问器；
    failpoint `SyncData` 置于计数之后，失败注入也能证明调用确实发生。

测试：`crate/engine/fkv_store/tests/durability_sync_order.rs`（新增）

- `SyncData@1!err=IoFail` 注入 data compact：断言 compact 返回 `IoFail`、
  manifest 字节与调用前完全一致（证明 manifest 未先落）、`max_fp_id` / root 回滚、
  搬迁页仍在 dirty 集合；reset 后重试成功，`data_sync_num` 恰好 +1 且 dirty 清空。
- segment compact 同构（`compact_segment_if_needed`）：注入失败后 manifest 不变、
  segment `max_fp_id` 回滚、dirty 非空；重试成功、`get_large` 数据一致。
- 负向验证：临时移除两处 `sync_data` 后本测试失败（compact 成功 → `unwrap_err` panic），
  确认测试真实覆盖顺序而非仅注释。

风险：compact 每次多一次数据 `fdatasync`（对齐 C++ `UpdateMeta` 的
WaitWrite → SyncData → FlushManifest，属预期成本）；失败语义沿用既有 abort 回滚。

## 复审（2026-11-05 r7）

**通过。** 行号较 DONE 漂移（`fkv_store/src/lib.rs` 热改，03:49 仍在被并行会话
触碰），按内容锚点逐处核实：

- `compact_file_li`（fkv_store/src/lib.rs:2504）在 :2546-2550 以
  `async { self.io.sync_data(tbl).await?; self.append_mapping_log(...).await }` 组合，
  注释 :2543-2545 点名 eng-89 崩溃窗口；失败经 `settle_mapper_batch(tbl, log_result.is_ok())`
  （:2565）走既有 abort。
- `compact_segment_file_li`（:3350）同构：:3391-3395，注释 :3390。
- 复用入口 `compact_if_needed`/`compact_segment_if_needed`/`compact_before_branch`/
  `maintain` 均只调这两个入口（:3156/:3163/:3268/:3342 亲读），自动继承，无第三条
  manifest 路径漏网（全文 `sync_data` 命中仅 put :1506 / clean_expired :2244 /
  两 compact :2547/:3392）。
- `fkv_io_compio`：`data_sync_num: AtomicU64`（lib.rs:670）+ 访问器（:985-986）；
  计数在 `fail_point!("SyncData")` 之前递增（:2185-2187），失败注入也能证明调用发生，
  与 DONE 声称一致。
- 测试 `tests/durability_sync_order.rs`：data/segment 两节均注入
  `SyncData@1!err=IoFail`（:73/:159），断言返回 `IoFail`（:77/:163）、
  `data_sync_num` 恰好 +1（:82/:168）、manifest 字节与调用前一致（:87/:169）、
  `max_fp_id`/root 回滚（:92-93/:174-177）、搬迁页仍在 dirty（:94/:178）、
  reset 重试成功且再 +1、dirty 清空、scan/get_large 数据完整（:100-106/:183-196）。
  断言真实覆盖「sync 先于 manifest」，非注释式测试（修复前 failpoint 不命中、
  `unwrap_err` 即 panic，文档头 :2-7 写明该负向逻辑）。
