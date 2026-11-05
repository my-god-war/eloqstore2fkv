# sto-62 fkv_api 请求枚举覆盖面 vs C++ RequestType；oneshot 完成契约无取消/超时语义

## 位置
- Rust: `crate/engine/fkv_api/src/lib.rs:7-84`（`Request` 17 变体）、完成契约 `crate/engine/fkv_shard/src/lib.rs:58-77`（`Envelope` + `ShardSender::request`）、回复 `227` 行（`let _ = envelope.reply.send(result)`）、OOM 重试 `186-193`
- C++: `include/eloq_store.h:40-65`（`RequestType` 23 种）、`576-590`（`DropRequest` 与 `DropTableRequest` 分离）、`136-195`（`KvRequest`：`callback_`、`reopen_retry_remaining_`、`oom_retry_remaining_`、`SetDsShard`）

## 现状
Rust `Request`：Put/Get/PutLarge/GetLarge/Scan/Reopen/Gc/Compact/Truncate/Floor/Drop/CleanExpired/AttachShard/DetachShard/CreateArchive/CreateBranch/DeleteBranch/Shutdown。

对照 C++ `RequestType` 的缺口与错位：
- 缺 **ListObject / ListStandbyPartition**（C++ 45-46 行）：standby/cloud 侧列举远端对象/分区的 API，`fkv_standby`/`fkv_cloud` 内部需要却无对外请求类型；
- 缺 **GlobalArchive / GlobalReopen / GlobalListArchiveTags / GlobalCreateBranch**（57-64 行）：跨分区 fanout 的全局操作类型；Rust 用 runtime 内部 fanout（ArchiveCrond）替代，但 `fkv_api` 无对应枚举，SDK 层无法表达「全局」语义；
- **Drop vs DropTable**：C++ 是两个请求（`DropRequest` 删某表数据、`DropTableRequest` 删整表，`eloq_store.h:576-590`）；Rust 只有 `Drop`（映射 `drop_table`），缺「drop 单 key/前缀」的请求语义；
- **Compact vs FileGc/LocalGc 粒度**：C++ 23 种里 `Compact`、`FileGc`、`LocalGc` 分离；Rust `Compact` + `Gc` 合并（`Gc` 一次跑 data+segment，`fkv_store.gc`），标签/指标归组不同（sto-60）；
- `Request::Put/PutLarge` 携带 `term: Option<u64>`（Rust 特有）：C++ 请求不带 term，term 由 shard attach 状态决定——Rust 的 per-request term 是 D31 的本地化设计，跨语言对账时无对应物。

完成契约（oneshot）：
- `ShardSender::request` 发 `Envelope{req, reply: oneshot::Sender}`，`rx.await` 收结果；调用方 drop future 即取消，但 shard 线程**仍会把请求完整执行到底**（`handle` 内无取消检查，回复 `let _ = reply.send(...)` 丢弃失败），取消只省调用方等待，不省执行；
- **无超时**：请求可能永远挂起（无 deadline 传递通道），`auto_reopen_pending_time_us` 只作用于显式 Reopen 请求的 ResourceMissing 重试（`fkv_shard:505-510`）；
- C++ `KvRequest` 是对象语义：`Wait()`/`callback_`/`IsDone()`，调用方持有请求对象、drop 前必须等待（stackful coroutine 下无「执行中取消」概念），重试次数 `reopen_retry_remaining_`/`oom_retry_remaining_` 挂在请求上；Rust 的重试是 shard 循环内部状态（`fkv_shard:186-193` OOM 重试、`store.auto_reopen` 重试），请求方不可见。

## C++ 实现
见上；C++ 请求生命周期与 store/shard 的 admit（AttachShard 状态）绑定，`AutoReopenRetry()` 由请求类型决定是否参与 reopen 重试（`eloq_store.h:141-144`）。

## 差异与风险
- **API 覆盖面缺口**：ListObject/ListStandbyPartition/Global* 请求类型缺失意味着 standby replica 列举远端、全局 archive 等 SDK 可见操作在 Rust 侧无对应请求（内部实现存在但不可编程触发或语义下沉）；
- **Drop 语义合并**：若未来补 DropRequest 单 key 语义，现有 `Drop` 的 drop_table 行为会被误用；
- **取消语义**：async 调用方（`fkv_sdk` future API）取消请求 = 白干一次写（RAII 回滚保证无泄漏，但 workload 浪费），与 C++「请求不可中途取消」的行为差异在崩溃/超时注入测试里会暴露为「请求已执行但调用方已放弃」；
- **无超时**：云模式挂起请求（S3 慢/死链）会永久占用 shard 队列位，队列满时新请求被拒（`send().await` 背压），无 deadline 兜底。

## 对齐建议
- `Request` 补 `ListObject`、`ListStandbyPartition`、`GlobalArchive/GlobalReopen/GlobalListArchiveTags/GlobalCreateBranch`（或明确文档化「不迁移，内部 fanout 替代」并在 ledger 标 ignored+reason）；
- 拆分或文档化 `Drop` 的 drop_table 语义；
- 完成契约：`Envelope` 增加 `CancelToken`（`Arc<AtomicBool>` 或 `Watch`），`handle` 长 IO 前检查取消；增加可选 deadline（`tokio::time::timeout` 包装 `rx.await`），超时返回 `KvError::Timeout`；
- 对账口径：指标 `request_type` 标签映射表见 sto-60。

## 性能优化建议
- 取消检查放 IO 边界（每次 `await` 前后），避免每页检查的开销。

## 严重度
P1（功能缺失/行为不一致）

## 审查结论（2026-11-04）

判定：**部分合理**。覆盖缺口全部重复，完成契约部分成立；已按小闭环落地取消令牌，其余拆分。

### 核验

- `Request` 现 18 变体（`crate/engine/fkv_api/src/lib.rs:22-100`）；`Envelope` /
  `ShardSender::request` / `RequestHandle` 在 `crate/engine/fkv_shard/src/lib.rs`。
- **覆盖缺口重复**：
  - `ListObject` / `ListStandbyPartition` / `ListArchiveTags` / `ReopenArchive` /
    `DeleteArchive` / `Prewarm` / `FileGc`-`LocalGc` 粒度 → 已登记
    `issue/todo/12a-per-table-request-gaps.md`（第 5、6 条）。
  - `GlobalArchive` / `GlobalReopen` / `GlobalListArchiveTags` / `GlobalCreateBranch`
    → 已登记 `issue/todo/12c-global-fanout-and-cloud-followups.md`。
  - standby 远端列举与同步编排 → `issue/todo/12b-standby-runtime-wiring.md`。
  - 本条不再重复拆分；剩余差异只做对账指向。
- **完成契约成立**：`ShardSender::request` 无 deadline；取消只省调用方等待，
  shard 仍会把已入队请求执行到底。
- **Drop vs DropTable**：`Request::Drop` → `Store::drop_table`；`Store::delete(tbl, key, ts)`
  已存在但无请求入口。仓库 `cpp/eloqstore` 为空（submodule 未拉），无法核对 C++
  `DropRequest` 精确定义，故只拆分待办、不臆造语义。

### 已落地（`sto-62a`）

- `Envelope` 增 `cancel: Arc<AtomicBool>`；`ShardSender::request` 返回私有
  `RequestHandle`（`Future` + `Drop` 置取消标志），超时统一用
  `compio::time::timeout` 包裹，不需要额外配置项。
- `process_envelope` / `run_read_batch` 执行前检查取消：未开始的请求直接回
  `NotRunning`，不白跑一次写；`Shutdown` 例外（必须真停机）。
- 回执点收敛到 `Envelope::send`，错误路径（`admit` 拒绝、OOM 耗尽、reopen 重试
  耗尽、写队列清理、取消、通道退出）均保证发送或由句柄 `Drop` 归一为
  `NotRunning`。
- 新增 `crate/engine/fkv_shard/tests/cancel.rs`：取消挂起读请求后重排跳过执行，
  `Op::Get` 计数不增长，shard 仍可服务；去掉取消检查后该测试失败（已验证有效）。

### 验证

- `cargo check -p fkv_api -p fkv_shard` 通过。
- `cargo nextest run -p fkv_api -p fkv_shard -p fkv_runtime --no-fail-fast`：51 通过 / 1 跳过。
- `cargo clippy -p fkv_api -p fkv_shard --all-targets -- -D warnings` 零警告。

### 拆分去向

- 已完成：`issue/done/sto-62a-oneshot-cancel-token.md`
- 待办：`issue/todo/sto-62b-request-deadline-and-backpressure.md`
- 待办：`issue/todo/sto-62c-drop-vs-droptable-semantics.md`
- 覆盖缺口：见 `12a` / `12b` / `12c`，本条不再另立。

## 收口（2026-11-05，本会话）

甄别：62a 代码落地属实（`Envelope.cancel` / `RequestHandle` / `tests/cancel.rs` 均在，
shard cancel 检查链完整）；62b、62c 拆分文件均在（62c 已 done，62b 留 todo）。
覆盖缺口已归口 12a/12b/12c。本文件无剩余开发量，按拆分完成移入 done/。

## 复审（2026-11-05 r6）

**通过（拆分归口闭合）。** 核实：62a done（本次复审通过）；62b done（本次复审通过）；62c done（本次复审通过）；覆盖缺口归口 12a（留 todo，在途会话）与 12b/12c（均 done 复审通过）。`Envelope`/`RequestHandle` 契约见 62a 证据，无剩余开发量挂本条。
