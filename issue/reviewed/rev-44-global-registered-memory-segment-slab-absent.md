
## DONE（2026-11-05，范围判定 + 残余项完成）

开发计划第 1 项已落地：`FixedSegmentIo::read_segment_fixed_li` 批量 fixed 读 +
`fkv_large::batch::read_batch_fixed` 批内并发、批间串行（`MAX_SEGMENTS_BATCH`
在飞上限）；`read_value_fixed` 由逐段串行改为批量提交，compio 后端以
`share_disjoint` 区间租约一次提交多个 READ_FIXED。测试见
`fkv_io_compio/tests/fixed.rs` / `fkv_large/tests/managed.rs`。

`GlobalRegisteredMemory` 本体按**范围外**关闭并登记 `COMPAT_BASELINE`：C++ 中它
是可选宿主集成设施（`kv_options.h` 明示留空即关闭），生产代码无 `SetEvictFunc`
调用点；fkv 以 per-shard 启动注册 + 调用方 `FixedBuffer` 动态注册覆盖注册 IO，
放不下回退 buffered IO。lock-free ABA 栈在 compio thread-per-core + `SlotLease`
下无对等需求。大 value 超过 fixed 缓冲时的瞬态内存上界差异保留在 baseline 的
排除项中，不构成正确性缺陷。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 特殊核对「裁剪为批量 fixed 读接口，裁剪理由在案」：

- 接口落地：`FixedSegmentIo::read_segment_fixed_li`（`crate/io/fkv_io/src/lib.rs:525`）、
  批内并发批间串行的 `read_batch_fixed`（`crate/core/fkv_large/src/batch.rs:89`，
  批上限唯一来源 `fkv_types::MAX_SEGMENTS_BATCH`，:26/:105/:118）、`read_value_fixed`
  改走批量提交（`crate/core/fkv_large/src/managed.rs:169`，注释 :153-154 述区间租约
  与分批）。
- 测试在盘：`crate/io/fkv_io_compio/tests/fixed.rs:223`
  `fixed_batch_read_submits_concurrent_ops`（:278 重叠槽位拒绝）、
  `crate/core/fkv_large/tests/managed.rs:253`
  `fixed_read_multi_batch_keeps_slack_region`。
- 裁剪理由在案：`COMPAT_BASELINE:27-33` 明列「`GlobalRegisteredMemory` 定长段注册内存
  slab / ABA 空闲栈 / `SetEvictFunc` 驱逐联动（rev-44）**不做 1:1**」，给出三条理由
  （可选宿主集成设施、无 `SetEvictFunc` 调用点、per-shard 注册 + 动态注册覆盖 + 回退
  buffered IO），并回指本源文件。裁剪有据，索引 00-index「按可选宿主设施范围外关闭」
  与之一致。
