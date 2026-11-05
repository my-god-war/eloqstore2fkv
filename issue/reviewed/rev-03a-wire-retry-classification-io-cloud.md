# rev-03a 把 `is_retryable` 接到 IO / 云重试点

## 位置

- `crate/io/fkv_io_compio/src/lib.rs:1644-1655`（`read_page` 的同路径重试）
- `crate/engine/fkv_cloud/src/lib.rs:893-916`（`put_manifest_with_retry`）
- `crate/engine/fkv_cloud/src/lib.rs:1637-1676`（`upsert_term`）
- `crate/foundation/fkv_error/tests/error.rs`

## 方案

1. `read_page`：把 `KvError::TryAgain` 手写 match 换成 `KvError::is_retryable()`。
   重试次数提成常量，去掉魔数 `3` / `2`。
   语义：`TryAgain | Busy | OpenFileLimit` 都属瞬时错误，只对同一路径重试。
2. 云 manifest：新增具名判据 `manifest_retryable(err) = err.is_retryable() || Timeout | CloudErr`，
   `put_manifest_with_retry` 用它；次数提成常量。
   注释写明是 `is_retryable` 的超集（durability anchor，对齐 C++ `SyncFile`）。
3. 云 term CAS：`Busy` 由 `is_retryable()` 覆盖，额外显式保留 `AlreadyExists`（CAS 冲突，
   不是瞬时错误）。次数提成常量。
4. 测试：
   - `fkv_error` 覆盖全部 20 个变体，仅 `OpenFileLimit | Busy | TryAgain` 为 true；
   - `fkv_cloud` 单测 `manifest_retryable`（含超集与不重试的 `Corrupted` /
     `OssInsufficientStorage`）。

## 验收

- `cargo check -p fkv_error -p fkv_io_compio -p fkv_cloud`
- `cargo nextest run -p fkv_error -p fkv_io_compio -p fkv_cloud`
- `cargo clippy -p fkv_error -p fkv_io_compio -p fkv_cloud --no-deps --all-features --all-targets -- -D warnings` 零警告

## DONE（2026-11-04）

- `read_page`：`TryAgain` 手写 match → `KvError::is_retryable()`，新增 `READ_RETRY_TIMES`。
- `fkv_cloud`：新增 `manifest_retryable()`（`is_retryable` 超集：+ Timeout/CloudErr），`upsert_term` 改用 `is_retryable()` + 显式 `AlreadyExists`；魔数提为常量。
- `fkv_task_reopen` / `fkv_shard::run_due_reopen`：注释说明只重试 `ResourceMissing`（窄于 `is_retryable`）的理由，行为不变。
- 测试：`fkv_error` 全变体分类覆盖；`fkv_cloud::tests::manifest_retryable_covers_transient_and_cloud_errors`。
- 验证：`cargo nextest run -p fkv_error -p fkv_task_reopen -p fkv_cloud -p fkv_io_compio -p fkv_shard` 全绿；`cargo clippy -p fkv_error -p fkv_cloud -p fkv_io_compio -p fkv_task_reopen -p fkv_shard --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_io_compio/src/lib.rs:92`（`const READ_RETRY_TIMES: usize = 3`）、
`:1947-1955`（`read_page` 同路径重试用 `err.is_retryable()`，注释说明覆盖
TryAgain/Busy/OpenFileLimit）；`fkv_error/tests/error.rs:8/:31`（全变体分类断言）。
fkv_cloud 侧 `manifest_retryable` 为 cloud lane 交叉项，本次抽验存在性未逐行（见 lane
报告注明）。
