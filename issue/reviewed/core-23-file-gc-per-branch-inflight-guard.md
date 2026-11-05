# core-23 文件 GC 缺 per-branch「least_unflushed」in-flight 高水位保护，manifest 未提交窗口内可删整文件

## 位置
- Rust: `crate/engine/fkv_task_gc/src/lib.rs:96-117`（`manifest_retained_file_page_set` 只做 active + branch + archive manifest 的引用并集）；`crate/engine/fkv_store/src/lib.rs:2219-2243`（`gc()` 再并入**本进程内存中** mapper 的映射，无任何 branch/term 高水位 guard）
- C++: `src/file_gc.cpp:118-165`（`SeedActiveBranchGuardFromInMemory`：用活跃表 allocator 的 `MaxFilePageId()` 高水位种出 branch guard，保护「已写入、manifest 尚未提交」的文件）；`:617-651`（`ProcessOneManifest` 把每个 manifest 的 `BranchFileRange` 折成 per-branch `least_unflushed_file_id_ / least_unflushed_seg_file_id_`（取最新 term 的 max），删除判定时该边界以下文件即使零引用也保留）

## 现状
Rust GC 的保留集合 = 各 manifest 引用 ∪ 本进程 mapper 引用。没有「branch 写入者可能已写数据页但增量 manifest 还没落盘」的保护边界。

## C++ 实现
两层保护：
1. in-memory 高水位（本进程 allocator 当前 max）；
2. per-branch least_unflushed（其它写入者/其它 shard 的最新已知边界，从各 manifest 的 BranchFileRange 折出）。

## 差异与风险
数据页已 sync、manifest 增量尚未落盘的窗口内，Rust GC 会把整文件判定为无引用并删除 → 增量落盘后引用悬空。
当前 fkv 单 actor 串行执行掩盖了本进程窗口；但 (a) branch 的最新 term 边界文件**现在**就无保护（另一写入者的 manifest 滞后），(b) todo 中 `03`/`eng-32` 并发化一落地即成为丢数据路径。

## 对齐建议
- `manifest_retained_file_page_set` 顺带折出 per-branch guard（replay 时已解析 `file_ranges`，成本低）；
- `gc_files_with_retained` 增加边界判定：file_id ≥ 该 branch 的 least_unflushed 即保留。

## 严重度
P1（丢数据窗口；并发化前必须修）

## 核验记录（2026-11-04）
Rust 侧两处实读确认无 guard；C++ `file_gc.cpp` 两段引用自子代理摘录（含函数名与语义注释），开发前需按行号复核。

## 审查结论（2026-11-04）

核验通过，问题成立：

- `manifest_retained_file_page_set`（现 `crate/engine/fkv_task_gc/src/lib.rs:231`）此前只返回两份 retained page set，完全没有从 `BranchManifestMetadata.file_ranges` 折出 per-branch 边界。
- `Store::gc`（现 `crate/engine/fkv_store/src/lib.rs:2217`）只并入本进程 mapper 引用，未做任何 branch/term 高水位判定。
- 仓库内没有 C++ `src/file_gc.cpp` 源码，无法复核行号；但 Rust 侧 `BranchFileRange` 已携带 `branch_name/term/max_file_id/max_segment_file_id`，语义足够实现 `ProcessOneManifest` 的折叠。
- 与本 issue 重叠的 `sto-54` 仍在 `issue/new/`，其「retained key 带 branch/term」部分未在本 issue 范围内实现；本 issue 只对齐 in-flight 高水位（BranchGuard）。

实现拆为单一 todo：`core-23-file-gc-per-branch-inflight-guard.md`。

注意：边界值 0 视为「无信息」不保护，否则 `truncate` 后单文件表会把整表文件当 in-flight，GC 无法回收（已由 `truncate_drop` 测试守住）。

## DONE（2026-11-04）

实现要点：

- `fkv_task_gc` 新增 `InflightGuard` / `BranchGuard`：
  - `fold` 从 `BranchManifestMetadata.file_ranges` 与 replay 后的 `max_fp_id` 折出 per-branch 最新 term + 未 flush 边界（同 branch 取最大 term，同 term 取最大 file id）。
  - 候选 file id 归属：取「上限不超过 file id 的最近区间」对应 branch；无归属时只由本进程 active branch 兜底，避免误保护已删除 branch 的残留文件。
  - data / segment 边界分开；边界 0 视为无信息（防止整表保留）。
- `manifest_retained_file_page_set` 增加 `pages_per_file_shift` / `segments_per_file_shift` 入参，返回 `(data_set, segment_set, InflightGuard)`。
- `gc_files_with_retained` / `gc_segment_files_with_retained` 增加 guard 判定：命中 retained 或 in-flight 边界即保留。
- `Store::gc` 用本进程 `mapper.max_fp_id()` / `segment_mapper.max_fp_id()` 补种 active branch guard。

测试文件：

- `crate/engine/fkv_task_gc/tests/inflight.rs`：边界折叠、term 取最新、branch 区间归属。
- `crate/engine/fkv_store/tests/gc_inflight.rs`：`gc_keeps_file_written_before_manifest_commit`（manifest 未提交窗口内文件不被删）。

验证命令与结果：

- `cargo check -p fkv_task_gc -p fkv_store -p fkv_gc -p fkv_task_reopen` 通过。
- `cargo check -p fkv_runtime -p fkv_cloud -p fkv_shard` 通过。
- `cargo nextest run -p fkv_task_gc -p fkv_store -p fkv_gc --no-fail-fast`：80 passed。
- `cargo nextest run -p fkv_runtime --no-fail-fast`：30 passed（首轮 `runtime_clean_expired_deletes_due_entries` 偶发超时，单跑与复跑均通过）。
- `cargo clippy -p fkv_task_gc -p fkv_store -p fkv_gc --no-deps --all-features --all-targets -- -D warnings` 零警告。

遗留：

- `sto-54` 的 branch/term retained key 维度未实现（本 issue 只做 in-flight guard）。

## 复审（2026-11-05 r6b）

通过。

- `BranchGuard`（`least_unflushed_file_id/segment`，`crate/engine/fkv_task_gc/src/lib.rs:24-27`）、`InflightGuard`（`:66`）、`fold`（`:112`，同 branch 取最大 term、同 term 取最大 file id `:137-142`）、候选归属「上限不超过 file id 的最近区间」（`:225-230`）均实现。
- 判定接线：`gc_files_with_retained` 走 `guard.retain_data(file_id)`（`:481`）；旧 term 越界保守保留（`:256`）。
- 测试断言覆盖修复：`crate/engine/fkv_task_gc/tests/inflight.rs:16-119`（7 个用例：边界保留、最新 term 折叠、区间归属、链中段保守）；`crate/engine/fkv_store/tests/gc_inflight.rs:12` `gc_keeps_file_written_before_manifest_commit` 实测「已写盘未提交 manifest 的 in-flight 文件 GC 后仍在」（`:34-38`），正是原缺陷场景。
