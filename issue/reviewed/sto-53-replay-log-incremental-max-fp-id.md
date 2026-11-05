# sto-53 ReplayLog 每条 log record 全表重算 max_fp_id，replay 复杂度 O(records × mapping)

## 位置
- Rust: `crate/engine/fkv_recovery/src/lib.rs:159-162`（`apply_log` 末尾调用 `max_file_page_id(&self.mapping_tbl)` / `max_file_page_id(&self.segment_mapping)`）、helper `crate/engine/fkv_recovery/src/lib.rs:83-91`
- C++: `src/replayer.cpp:252-256` 与 `296-300`（`ReplayLog` 对每条 delta 只做 `max_fp_id_ = std::max(max_fp_id_, fp_id + 1)`）

## 现状
Rust `apply_log` 每应用一条 log record（含可能成千上万条），都完整扫描 data mapping 表与 segment mapping 表找最大 file page id。`fkv_task_reopen::replay_manifest_bytes`（`crate/engine/fkv_task_reopen/src/lib.rs:149-160`）对 active manifest 全量 replay，branch/archive manifest 的 retained set 汇总（`crate/engine/fkv_task_gc/src/lib.rs:102-107`）同样走 `replay_manifest_bytes`——GC/compaction 的 manifest 枚举因此是 O(manifest_num × records × mapping_size)。

## C++ 实现
`Replayer::ReplayLog` 在解析每个 delta 的 `fp_id` 时原地更新 `max_fp_id_`（`replayer.cpp:252-256`），O(delta)；snapshot 路径只在 `DeserializeSnapshot` 读一次持久化的 `max_fp_id`（151 行），不从 mapping 表重算。

## 差异与风险
- 功能上结果一致（取 max 是幂等的），但：
  - 大 manifest（接近 8 MiB manifest_limit、上万条 log）冷启动 replay 与每次 GC 的 `AugmentRetainedFilesFromBranchManifests` 等价路径都变成平方级开销；
  - C++ 侧该成本已消除（ProcessOneManifest 每 manifest 一次 replay），Rust 在 GC 枚举全部 manifest 时放大为每 manifest 全表扫描。
- 不算数据损坏，是冷启动/后台任务延迟与 CPU 争用的回归点。

## 对齐建议
- `apply_log` 增量维护 `max_fp_id`：解析每条 `(page_id, value)` delta 时按 C++ 方式 `max_fp_id = max(max_fp_id, decode_id(value) + 1)`（当前已算 `max_file_page_id` 是因为 `MappingTbl::set` 不回调；直接在内层 while 循环里更新即可）；
- segment 侧同理（`fkv_recovery/src/lib.rs:153-157`）；
- `from_snapshot` 保留现有「持久化值 ∧ 表内最大值」的 max 语义（比 C++ 更严，无害），但只在 snapshot 应用时做一次。

## 性能优化建议
- 若 `MappingTbl` 增加 `max_file_page_id` 惰性缓存（`set` 时清零），`from_snapshot` 的 `max_file_page_id` 也变 O(1) 摊还。

## 严重度
P2（性能/健壮性）

## 审查结论（2026-11-04）

**判定：合理，需实现。**

逐条核验：

- `apply_log` 末尾确有两次全表扫描（`max_file_page_id(&self.mapping_tbl)` / `max_file_page_id(&self.segment_mapping)`），位于 `crate/engine/fkv_recovery/src/lib.rs:159-162`，与 issue 描述一致。
- `max_file_page_id` 走 `MappingTbl::value_li()`（`crate/core/fkv_mapping/src/lib.rs:190`），该函数**每次调用都 `collect` 出一个 `Vec<u64>`**，所以代价是「每条 record 两次全表拷贝 + 扫描」，比 issue 估计更差。
- `replay_manifest_bytes` 对每条 log 调 `apply_log`，GC 的 manifest 枚举因此被放大为 O(manifests × records × mapping)。
- 语义上 `max` 幂等，改成增量维护不改结果：`from_snapshot` 已把持久化 `max_fp_id` 与表内最大值取 max，`apply_log` 从该值出发增量更新即可覆盖原「全表取 max」的全部信息。

## 方案（单文件，一次改动）

1. `crate/engine/fkv_recovery/src/lib.rs::apply_log`
   - mapping 内层 while：`set` 后若 `is_file_page_id(value)`，就地 `self.max_fp_id = self.max_fp_id.max(decode_id(value) + 1)`。
   - segment 内层 while：同式更新 `self.max_segment_fp_id`。
   - 删掉两处末尾全表重算。
   - 说明注释对齐 C++ `Replayer::ReplayLog`（`replayer.cpp:252-256`）。
2. `max_file_page_id` 保留，仅供 `from_snapshot` 单次使用（快照路径语义不变：持久化值 ∧ 表内最大值）。
3. `decode_id(value) + 1` 不会溢出：`is_file_page_id` 保证低 3 位为 1，`decode_id(value) <= (u64::MAX - 1) >> 3`。

## 验收

- `crate/engine/fkv_recovery/tests/recovery.rs` 新增用例：
  - `apply_log` 后 `max_fp_id` / `max_segment_fp_id` 跟随 delta 单调抬升；
  - 非 `FilePageId` 值（`PageId` / `Invalid`）不参与抬升；
  - 从 snapshot（含持久化 `max_fp_id` 高于表内值）继续 replay 时保持较大者。
- `cargo nextest run -p fkv_recovery -p fkv_task_reopen -p fkv_store` 全绿。
- `cargo clippy -p fkv_recovery --no-deps --all-features --all-targets -- -D warnings` 零警告。
- replay 语义不变：mapping log 顺序、torn tail、branch term 相关既有测试全部保持通过。

## DONE（2026-11-04）

- `crate/engine/fkv_recovery/src/lib.rs`
  - 新增 `bump_max_fp_id(&mut FilePageId, u64)`：仅 `is_file_page_id` 时抬升，`decode_id+1` 无溢出。
  - `apply_log` mapping / segment 两个内层 while 就地更新 `max_fp_id` / `max_segment_fp_id`，删除末尾两次 `max_file_page_id` 全表扫描（原先每条 record 还额外 `value_li()` 整表拷贝一份 Vec）。
  - `max_file_page_id` 收敛为 `from_snapshot` 专用的单次扫描，语义不变（持久化值 ∧ 表内最大值）。
- `crate/engine/fkv_recovery/tests/recovery.rs`：新增 3 个用例
  - `log_replay_increments_max_fp_id`（单调抬升，`PageId` / `Invalid` 不参与）
  - `log_replay_keeps_snapshot_max_fp_id`（快照持久化高水位优先）
  - `log_replay_increments_max_segment_fp_id`（segment delta 增量维护）

验证：

- `cargo nextest run -p fkv_recovery -p fkv_task_reopen -p fkv_store --no-fail-fast` → 93 passed。
- `cargo clippy -p fkv_recovery --no-deps --all-features --all-targets -- -D warnings` → 零警告。
- replay 语义不变：mapping log 顺序、torn tail、branch term 既有测试全绿。

## 复审（2026-11-05 r6）

通过。`fkv_recovery/src/lib.rs:114-118`（`bump_max_fp_id`：仅 `is_file_page_id` 时抬升，
注释论证无溢出）、`:173-178`（mapping 内层 while 就地更新）、`:188-193`（segment 同），
`apply_log` 已无末尾全表扫描；`max_file_page_id` 仅剩 `from_snapshot` 单次使用
（:132/:144）。测试 `fkv_recovery/tests/recovery.rs:108`（单调抬升、PageId/Invalid 不
参与）、`:136`（快照高水位优先）、`:163`（segment 增量）断言与验收逐项对应。
