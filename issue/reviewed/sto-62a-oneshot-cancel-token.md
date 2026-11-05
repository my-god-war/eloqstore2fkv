# sto-62a oneshot 完成契约：取消令牌

## 来源

`issue/todo/sto-62-fkv-api-request-coverage-and-oneshot-completion-contract-divergence.md`
的完成契约部分（覆盖缺口部分重复，见 12a/12b/12c，不在此实现）。

## 位置

- `crate/engine/fkv_shard/src/lib.rs`
  - `Envelope` / `Envelope::canceled` / `Envelope::send`
  - `RequestHandle`（`Future` + `Drop`）
  - `ShardSender::request` / `ShardSender::enqueue`
  - `process_envelope`、`run_read_batch`、`fail_pending`
- `crate/engine/fkv_shard/tests/cancel.rs`（新增）

## 落地内容

1. `Envelope` 增加 `cancel: Arc<AtomicBool>`；`ShardSender::request` 返回私有
   `RequestHandle`，`Drop` 时置取消标志，`poll` 时把回执通道的 `Canceled` 归一为
   `KvError::NotRunning`。
2. `process_envelope` / `run_read_batch` 在 `admit` / `prepare_table` 之前检查取消：
   未开始的请求直接回 `NotRunning`，不再白跑一次写。`Shutdown` 不受取消影响。
3. 回执统一走 `Envelope::send`，所有错误路径（admit 拒绝、OOM 重试耗尽、
   `ResourceMissing` 重试耗尽、写队列 `Drop`/`DetachShard`/`Shutdown` 清理、取消、
   回执通道断开）都有确定的完成结果。
4. 不新增配置项：超时由调用方用 `compio::time::timeout` 包裹 `request` 实现，
   超时 drop 句柄即触发取消。

## 语义边界

- 取消只在请求**开始执行前**生效（IO 边界检查）。已在执行的请求跑完，与 C++
  「请求不可中途取消」一致。
- 写请求在写队列里排队时被取消：`gate` 仍会取出并走 `process_envelope`，
  由取消检查跳过，队列位随 `gate.finish` 释放。

## 验收

- [x] `cargo check -p fkv_api -p fkv_shard` 通过。
- [x] `fkv_shard::cancel canceled_parked_read_is_skipped`：Cloud 模式下读请求挂起 →
      调用方超时取消 → manifest 出现后重排被跳过（`Op::Get` 计数不增长），后续请求
      正常返回。
- [x] 去掉取消检查后该测试失败（反向验证通过）。
- [x] `cargo nextest run -p fkv_api -p fkv_shard -p fkv_runtime --no-fail-fast` 全绿。
- [x] `cargo clippy -p fkv_api -p fkv_shard --all-targets -- -D warnings` 零警告。

## 重叠

- 覆盖缺口（ListObject / Global* / Prewarm / archive tag 等）：
  `issue/todo/12a-per-table-request-gaps.md`、`12c-global-fanout-and-cloud-followups.md`。
- 请求级 deadline 与队列背压：`issue/todo/sto-62b-request-deadline-and-backpressure.md`。
- `Drop` 语义拆分：`issue/todo/sto-62c-drop-vs-droptable-semantics.md`。

## DONE（2026-11-04）

## 复审（2026-11-05 r6）

**通过。** 核实：`fkv_shard/src/lib.rs:162` `Envelope.cancel: Arc<AtomicBool>`、:170 `canceled()`、:194-213 `RequestHandle`（Drop 置取消 + Canceled 归一 NotRunning）；:1445 `process_envelope` 执行前取消检查（Shutdown 除外）、:1656 读批路径同检。测试 `tests/cancel.rs:110` `canceled_parked_read_is_skipped`：:125/:131 用 `Op::Get` 计数前后断言被取消请求未执行——断言实质覆盖取消语义。
