# rev-03b reopen 重试收窄语义写明理由

## 位置

- `crate/engine/fkv_task_reopen/src/lib.rs:125-150`（`auto_reopen_with_size_with_sleeper`）
- `crate/engine/fkv_shard/src/lib.rs:452-476`（`run_due_reopen`）

## 方案

只改注释，不改行为：两处 reopen 重试只认 `KvError::ResourceMissing`，
而 `ResourceMissing` **不在** `KvError::is_retryable()` 集合内。
理由：这是「manifest 暂不可见 → 等待后重试」的可见性语义，不是通用瞬时错误分类；
套用 `is_retryable()` 会额外重试 `TryAgain/Busy/OpenFileLimit`，行为变宽且与 C++ 不一致。
注释需明确「窄于 is_retryable，刻意为之」。

不改 `fkv_store::auto_reopen_with_sleeper`：已由 `issue/todo/eng-39b` 列为待删死代码。

## 验收

- `cargo check -p fkv_task_reopen -p fkv_shard`
- `cargo nextest run -p fkv_task_reopen -p fkv_shard`
- 相关 clippy 零警告

## DONE（2026-11-04）

- `read_page`：`TryAgain` 手写 match → `KvError::is_retryable()`，新增 `READ_RETRY_TIMES`。
- `fkv_cloud`：新增 `manifest_retryable()`（`is_retryable` 超集：+ Timeout/CloudErr），`upsert_term` 改用 `is_retryable()` + 显式 `AlreadyExists`；魔数提为常量。
- `fkv_task_reopen` / `fkv_shard::run_due_reopen`：注释说明只重试 `ResourceMissing`（窄于 `is_retryable`）的理由，行为不变。
- 测试：`fkv_error` 全变体分类覆盖；`fkv_cloud::tests::manifest_retryable_covers_transient_and_cloud_errors`。
- 验证：`cargo nextest run -p fkv_error -p fkv_task_reopen -p fkv_cloud -p fkv_io_compio -p fkv_shard` 全绿；`cargo clippy -p fkv_error -p fkv_cloud -p fkv_io_compio -p fkv_task_reopen -p fkv_shard --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过（附记录瑕疵）。目标注释已存在：`fkv_shard/src/lib.rs:1009`
（「只重试 `ResourceMissing`、不用 `KvError::is_retryable`：这是 manifest 可见性……」），
行为未变。两处瑕疵记录在案：本文件 DONE 段是 rev-03a 内容的复制（讲 read_page/fkv_cloud，
与 03b 无关）；其「位置」引用的 `fkv_task_reopen/src/lib.rs:125-150
auto_reopen_with_size_with_sleeper` 已被 eng-87 删除，reopen 重试唯一生产落点现仅在
`fkv_shard`（`ReopenQueue`/`run_due_reopen`）。不影响结论：注释存在、行为一致。
