# rev-03 `KvError::is_retryable` 生产零消费，重试策略分散手写

## 位置

- Rust：`crate/foundation/fkv_error/src/lib.rs:58-60`

```rust
pub const fn is_retryable(&self) -> bool {
  matches!(self, Self::OpenFileLimit | Self::Busy | Self::TryAgain)
}
```

- 唯一引用在测试：`crate/foundation/fkv_error/tests/error.rs:5-9`。
- C++：`eloqstore/include/error.h:87-98`（`IsRetryableErr`，同一集合：`OpenFileLimit` / `Busy` / `TryAgain`）。

## 现状

生产代码里 retry 逻辑各自手写，不用这个分类，例如：

- `fkv_shard` 的 reopen / auto-reopen 重试（按错误种类 match，而非 `is_retryable`）；
- `fkv_io_compio::read_page` 对 `TryAgain` 的 3 次重试（`:1400-1415` 一带）；
- 云 term CAS 的 10 次重试。

即：分类定义与「哪里该重试」的判定是两套，容易漂移（例如 `Timeout` / `Busy` 在新路径上被漏重试，或不该重试的 `Corrupted` 被重试）。

## 差异与风险

- 与 C++ 的差别不是「集合不同」（集合一致），而是 **没有单一 retry 判据**：C++ `IsRetryableErr` 被实际重试路径消费；Rust 的分类是死代码。
- 后续加 OOM/超时/限流重试时，每处各写一遍 `match`，行为不一致且易漏。

## 对齐建议

1. 把重试入口统一到 `is_retryable()`（或一个 `retry_policy(err) -> Option<RetryKind>` helper），至少让 reopen / IO / 云 CAS 三处共用同一判据。
2. 若某处确实需要更窄/更宽的重试集合，显式写明理由而不是再写一个 `match`。
3. 与 `eng-78`（死代码清单）重叠：本条不是「删掉就好」，而是「接上它」。

## 性能优化建议

无。

## 严重度

P2（行为一致性 / 可维护性；当前无直接 bug，但重试判定已开始分叉）

## 核验记录

- 本人实读：`fkv_error/src/lib.rs:33-61`；全仓 `grep is_retryable` 仅命中 `tests/error.rs`。
- 手写重试点（`fkv_shard` reopen、`fkv_io_compio` read_page TryAgain）为摘录，开发时按实际行号复核。

## 审查结论（2026-11-04）

核验结果：**部分合理，采纳并修正后实现**，不拒绝。

### 属实

- `KvError::is_retryable` 生产零消费：全仓仅 `crate/foundation/fkv_error/tests/error.rs:5` 引用。
- 分类集合与 C++ 对齐：`OpenFileLimit | Busy | TryAgain`（`fkv_error/src/lib.rs:58-60`）。
- 重试判定分散在多处手写 `match`：
  - `fkv_io_compio::read_page`：`crate/io/fkv_io_compio/src/lib.rs:1644-1655`（issue 写 `1400-1415`，行号已漂移）；
  - `fkv_cloud::put_manifest_with_retry`：`crate/engine/fkv_cloud/src/lib.rs:893-916`；
  - `fkv_cloud::upsert_term`：`crate/engine/fkv_cloud/src/lib.rs:1637-1676`。

### AI 审查有误 / 需修正

1. **「三处共用同一判据」不能直接照做**，行为会漂移：
   - reopen / auto-reopen 只重试 `ResourceMissing`（`fkv_task_reopen/src/lib.rs:147`、`fkv_shard/src/lib.rs:468`），
     `ResourceMissing` **不在** `is_retryable` 集合内，套用后重试集合反而变宽。
     这是「manifest 可见性等待」语义，不是瞬时错误，应保留并在注释写明理由。
2. `fkv_cloud::put_manifest_with_retry` 是 `is_retryable` 的**超集**（额外 `Timeout | CloudErr`），
   属于 durability anchor 的刻意放宽，不能收敛成 `is_retryable()`。
3. `upsert_term` 重试 `Busy | AlreadyExists`，`AlreadyExists` 是 CAS 冲突而非瞬时错误，需显式追加。
4. 「C++ `IsRetryableErr` 被实际重试路径消费」无法在本仓库验证（`eloqstore` C++ 源码不在仓内）。
5. 与 `issue/todo/eng-39b-store-auto-reopen-dead-code.md` 重叠：`fkv_store::auto_reopen_with_sleeper`
   已列为待删死代码，本条不改该函数。

### 采纳方案

- 不删除 `is_retryable`（它是有 C++ 对齐依据的公共判据），接线到真正常态重试点：
  `read_page`、云 manifest、云 term CAS。
- 各点相对公共判据的加减项抽成具名函数 / 注释，附单元测试。
- 窄集合点（reopen）只加注释说明理由，不改行为。

## DONE（2026-11-04）

- `read_page`：`TryAgain` 手写 match → `KvError::is_retryable()`，新增 `READ_RETRY_TIMES`。
- `fkv_cloud`：新增 `manifest_retryable()`（`is_retryable` 超集：+ Timeout/CloudErr），`upsert_term` 改用 `is_retryable()` + 显式 `AlreadyExists`；魔数提为常量。
- `fkv_task_reopen` / `fkv_shard::run_due_reopen`：注释说明只重试 `ResourceMissing`（窄于 `is_retryable`）的理由，行为不变。
- 测试：`fkv_error` 全变体分类覆盖；`fkv_cloud::tests::manifest_retryable_covers_transient_and_cloud_errors`。
- 验证：`cargo nextest run -p fkv_error -p fkv_task_reopen -p fkv_cloud -p fkv_io_compio -p fkv_shard` 全绿；`cargo clippy -p fkv_error -p fkv_cloud -p fkv_io_compio -p fkv_task_reopen -p fkv_shard --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过（父条目）。rev-03a 接线与 rev-03b reopen 窄集注释均已按当前源码复核，见各自
r6 复审小节。
