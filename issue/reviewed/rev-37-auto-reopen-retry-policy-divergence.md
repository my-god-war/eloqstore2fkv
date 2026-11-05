# rev-37 auto-reopen 重试策略偏离 C++：driver 多轮重试且外部 Reopen 请求被 park

## 位置
- Rust: `crate/engine/fkv_shard/src/lib.rs:752-800`（`ReopenQueue::park` / `entry_index_or_insert`）、`:807-834`（`run_due_reopen` 的 `attempts_left` 多轮重试）、`:1176-1185`（`process_envelope` 的 park 条件把 `Request::Reopen` 一并挂起）
- C++: `src/storage/shard.cpp:303-328`（`EnqueueReopenWaiter`）、`:330-350`（`EnqueueDelayedReopenRequest`）、`:352-380`（`PromoteReadyDelayedReopenRequests`）、`:382-401`（`CompleteReopenWaiters`）、`:1384-1395` + `:1494-1498`（`OnTaskFinished` 的 auto_reopen 分支）、`:593-606`（`OnReceivedReq`：Reopen 类型请求不允许 park）

## 现状
fkv 中，遇 `ResourceMissing` 的 `Request::Reopen` 与读请求都进 `ReopenQueue`；driver
失败仍 `ResourceMissing` 时按 `attempts_left`（=`auto_reopen_retry_times`）逐轮重新到期
重试，每轮间隔 `pending_time`。首条 `Reopen` 请求被选为 driver，等待回执直到重试预算
耗尽。

C++ 的重试预算模型不同：
- **内部 driver 只延迟尝试一次**。`EnqueueReopenWaiter` 建 `pending_reopens_` 状态，内部
  `ReopenRequest` 带 `pending_time` 入 `delayed_requests_` 堆；到期后经
  `PromoteReadyDelayedReopenRequests` 提升为该分区写队列队首执行。**一次**。
- driver 失败（`ReopenRequest::AutoReopenRetry()` 为 false，`eloq_store.h:141`）走
  `OnTaskFinished` 的正常完成分支：`pending_reopens_` 状态被 erase，waiter 全部以
  driver 的错误（`ResourceMissing`）**立即 SetDone 失败**（`CompleteReopenWaiters`，
  shard.cpp:382-391）——不重排队、不再延迟。
- **外部 `ReopenRequest` 从不 park**：`OnTaskFinished` 的 auto_reopen 分支要求
  `AutoReopenRetry()==true`（只有 Read/Floor/Scan 重写为 true，`eloq_store.h:218/272/300`）；
  `OnReceivedReq` 对带 Reopen 标志的 Reopen 类型请求直接 `InvalidArgs`（shard.cpp:595-603）。
  外部 Reopen 遇远端无 manifest 立即以 `ResourceMissing` 回执。
- 读请求自己的 `reopen_retry_remaining_` 预算只作用于「读再次失败重新入 waiter 队列」
  这一层（driver 成功后重投的读再次 ResourceMissing 才再消耗一次），不是 driver 轮次。

## C++ 实现
`Shard::OnTaskFinished`（1384-1395）：`ResourceMissing` 且 `req->AutoReopenRetry()` 且
`reopen_retry_remaining_ > 0` → `--` 预算、`EnqueueReopenWaiter`；否则按普通错误完成。
driver 结果处理在 1437-1456：无论成败先 erase 状态并把 waiters 交给
`CompleteReopenWaiters(err)`；成功→重投（`SendRequest`），失败→以同错完成。

## 差异与风险
- 外部 `Reopen` 调用方视角：C++ 立即拿到 `ResourceMissing`（由调用方自己决定退避重
  试）；fkv 会挂 `retry_times × pending_time` 才失败。上层自带退避时形成双层等待，
  且 fkv 的 `Runtime::reopen` 默认 request_timeout 可能先把调用方切成 Timeout，错误
  类型都不同。
- 读请求视角：C++ 最坏等一个 `pending_time` 后拿到 `ResourceMissing`；fkv 最坏等
  `retry_times × pending_time`。
- driver 多轮重试本身对「manifest 短暂不可见」更鲁棒，但这是行为决策而非实现错误，
  应显式登记；当前两者都未在 API 文档声明，C++ 兼容层按 C++ 延迟预期写超时会误配。

## 对齐建议
- 方案 A（严格对齐）：`run_due_reopen` 首次失败即 `entry.fail(ResourceMissing)`；
  多轮重试改为 fkv 扩展开关（如 `auto_reopen_retry_times > 1` 时才启用，默认 1 对齐
  C++）——注意 fkv_config 的 `auto_reopen_retry_times` 与 C++ 同名但当前语义不同，需
  在注释与 INI 文档标明。
- 外部 `Request::Reopen` 建议对齐 C++：不作为 driver park（或仅作为 waiter 挂靠已有
  driver），失败立即回执，避免调用方观察到长尾。

## 严重度
P2

## 核验记录
- 读 `fkv_shard/src/lib.rs` 的 `PendingReopen`/`ReopenQueue`/`run_due_reopen`
  （696-834）与 `process_envelope` park 条件（1176-1185）。
- 读 C++ `shard.cpp` 全文中 reopen 相关：`OnReceivedReq`（576-621）、
  `EnqueueReopenWaiter`（303-328）、`EnqueueDelayedReopenRequest`（330-350）、
  `PromoteReadyDelayedReopenRequests`（352-380）、`CompleteReopenWaiters`（382-401）、
  `OnTaskFinished`（1355-1532）。
- 读 `eloq_store.h`：`AutoReopenRetry` 默认 false（141），Read/Floor/Scan 重写为 true
  （218/272/300）；`reopen_retry_remaining_` 在 `ExecAsyn` 播种（1171）。
- 确认 fkv 无「driver 一次失败即 fail waiters」路径：`run_due_reopen` 仅
  `attempts_left == 0` 才 `fail`。

## 甄别（2026-11-05 r6）

结论：**采纳**（差异属实且未登记；但开发计划须尊重 eng-39/39a 两项已定决策，见下）。

### 证据核实（Rust 行号已随并行会话热改漂移，按内容复核）
- Rust `crate/engine/fkv_shard/src/lib.rs`（当前行号）：
  - `PendingReopen`（:911-922，`attempts_left: u8` :916）；`ReopenQueue::park`
    （:972-989，:977 `matches!(envelope.req, Request::Reopen { .. })`，:982-987
    首个 `Reopen` 且无 driver/无 waiter → 外部 driver，否则挂 waiter）；
    `entry_index_or_insert`（:992-1007，新建 `attempts_left = retry_times` :1002，
    `retry_times == 0` 不挂立即失败）。
  - `run_due_reopen`（:1022-1049）：`Err(ResourceMissing) if attempts_left > 0`
    （:1041-1045）→ 减预算、`due = now + pending_time` 重新入队——多轮重试属实；
    仅预算耗尽或非 `ResourceMissing` 才 `entry.fail(err)`。
  - `process_envelope` park 条件（:1512-1519）：`Request::Reopen` 一旦
    `ResourceMissing` **无条件** park（第一臂不受 `reopen_on_missing` 门控）；读请求
    才受 `reopen_on_missing && is_auto_reopen_read`（Get/GetLarge/Floor/Scan，
    :1332-1337）门控。
  - `Request::Reopen` 分支（:2269-2277）单次执行，重试交事件循环——与意见描述一致。
- C++（`src/storage/shard.cpp`）：`EnqueueReopenWaiter`（:303-328，首行
  `CHECK(req->Type() != RequestType::Reopen)`）只建**内部** driver 并按
  `pending_time` 延迟入堆；`PromoteReadyDelayedReopenRequests`（:352-380）到期提升
  **一次**；`CompleteReopenWaiters`（:382-401）失败即全部 `SetDone(err)`。
  `OnTaskFinished`（:1382-1396）auto_reopen 分支要求
  `req->AutoReopenRetry() && reopen_retry_remaining_ > 0`；`ReopenRequest` 不覆写
  `AutoReopenRetry()`（`eloq_store.h:141-144` 默认 false，仅 Read/Floor/Scan 覆写
  true :218/:272/:300）→ 内部 driver 自身失败不重排队，走 :1437-1456 分支 erase
  状态 + `CompleteReopenWaiters(err)` 立即连坐。`OnReceivedReq`（:593-606）对带
  Reopen 标志的 Reopen 类型请求直接 `InvalidArgs`。外部 `ReopenRequest` 在
  `ProcessReq`（:1072-1089）`CHECK_EQ(PendingTime(), 0)` 后**立即**执行并经
  `SetExternalDriver` 成为 driver——失败也是一次即回 `ResourceMissing`。
- 同名配置同默认不同语义属实：C++ `kv_options.h:422/429`
  （`auto_reopen_retry_times = 10` / `10'000'000us`）语义是**每读请求**预算
  （`ExecAsyn` 播种，`eloq_store.h:1171`，仅在「driver 成功后重投的读再次失败」时
  消耗）；fkv `fkv_config/src/lib.rs:249-251` 同名同默认（10 / 10s）语义是
  **driver 重试轮数**。
- **勘误一处**：「fkv 的 `Runtime::reopen` 默认 request_timeout 可能先把调用方切成
  Timeout」不成立——`fkv_runtime/src/lib.rs:1262` 默认 `request_timeout: None`
  （「未配置时与旧行为一致」，无超时）。默认下调用方会挂满整个
  `retry_times × pending_time`（默认 100s），只有用户显式
  `with_request_timeout` 且短于预算时才出现 Timeout 截断；风险方向不变甚至更重。

### 与 eng-39a / eng-39 的边界（不可推翻的已定决策）
- eng-39a（reviewed）已**拒绝**「同表已有 driver 时外部 `Reopen` 回 `InvalidArgs`」：
  fkv 把后续 `Reopen` 挂 waiter，driver 成功后回 `Response::Reopen`，「等待方拿到
  成功而不是报错，语义更强」。本条的「外部 Reopen 不被 park」**不得**重新引入
  InvalidArgs / 禁止挂 waiter；意见建议中「或仅作为 waiter 挂靠已有 driver」与该
  决策兼容。
- eng-39 阶段 A2（done）已定「**首试立即** + 再等 `pending_time`」（eng-39a 待做 4
  同步拒绝「首次尝试也延迟」）：本条不触碰该时序，只涉重试**轮数**与失败回执时延。
- eng-39 A2 的「调用方语义不变」是把旧 `auto_reopen_with_sleeper` 的 N 轮重试原样
  迁入 `ReopenQueue`（重构中立），**从未对照 C++「driver 单次 + 立即连坐」登记过**；
  eng-39c（实现载体）与 `fkv_shard/tests/cloud_reopen.rs:161`（重试耗尽 driver+waiter
  同错）只是固化了现状。故「driver 轮数语义 + 外部 driver 失败时延 + 同名配置语义」
  三点是**未登记的新决策请求**，非 eng-39a 重复（rev-34 甄别亦明确本条与其正交）。

### 开发计划（决策优先，再动码）
1. **登记差异**（无争议，先做）：`doc/architecture/05-persistence-and-recovery.md`
   补「fkv driver 按 `retry_times` 多轮重试 vs C++ 单次延迟尝试即连坐」；
   `fkv_config` 两个 `auto_reopen_*` 字段注释与 INI 文档标明与 C++ 同名不同义
   （fkv=driver 轮数，C++=每读请求预算）。
2. **行为决策**（二选一，须在 md 记录理由）：
   - 方案 A（严格对齐）：`run_due_reopen` driver 首次 `ResourceMissing` 即
     `entry.fail`；`auto_reopen_retry_times>1` 作为 fkv 扩展开关。注意会改既有
     调用方语义与测试：`fkv_runtime/tests/auto_reopen.rs::
     runtime_reopen_exhausts_retries`、`fkv_shard/tests/cloud_reopen.rs`（重试
     耗尽用例）需同步改。
   - 方案 B（保留为刻意扩展，与 eng-39a「更强语义」取向一致）：保留多轮，仅补 1
     的登记 + 文档声明「上层退避需自行叠加」；外部 `Reopen` 保持现状（挂 waiter
     对齐 eng-39a，driver 挂满预算）。
3. 无论 A/B：修正本文件「差异与风险」中 request_timeout 的错误表述（默认 None）。

### 热点文件标注
- `crate/engine/fkv_shard/src/lib.rs`：并行会话热改中，行号持续漂移，接手时按
  符号（`ReopenQueue`/`run_due_reopen`/`process_envelope`）定位。
- `crate/engine/fkv_runtime/src/lib.rs`、`crate/foundation/fkv_config/src/lib.rs`、
  `doc/architecture/05-persistence-and-recovery.md`：改动前先确认无并行会话占用。

## DONE（2026-11-05）

**状态：代码 + 测试闭环（对齐 C++ 方案 A），issue 关闭。**

### 改动（`crate/engine/fkv_shard/src/lib.rs`）

1. **每请求剩余次数**：`Envelope` 增 `reopen_retry: Option<u8>` +
   `take_reopen_retry(seed)`（首次按 `auto_reopen_retry_times` 播种，每次挂起
   消耗 1）。`ReadSlot` 随读任务带回预算，driver 成功重排后读再次
   `ResourceMissing` 才消耗下一轮；预算是 0 立即回执，不挂 waiter
   （对齐 C++ `OnTaskFinished` 先判 `reopen_retry_remaining_ > 0`）。
2. **driver 单次尝试**：`run_due_reopen` 去掉 `attempts_left` 多轮重试；到期
   `handle(Reopen)` 一次，成功重排 waiter、失败（含仍 `ResourceMissing`）立即
   连坐全部 waiter（对齐 `CompleteReopenWaiters(err)`）。
3. **外部 Reopen 不无条件 park**：`park` 在无同表条目时不建 driver，直接交回
   `ResourceMissing` 回执（对齐 C++ 外部 `ReopenRequest` 一次即回）；同表已有
   driver 时仍挂 waiter（保留 eng-39a「等待方拿到成功」的已定决策）。空 tag 的
   `ReopenArchive` 同语义。删除已无意义的 `PendingReopen.driver` 外部回执字段。
4. **延迟堆合并**：`ReopenQueue` 由 `Vec` 线性扫描改为
   `BinaryHeap<DelayedReopen>`（按 `due` 反转的最小堆）；`park` 合并同表 waiter
   （每表至多一条延迟项），`next_due` 为 O(1) peek，写让路时重新入堆不消耗预算。

### 测试

- `fkv_shard/src/lib.rs` 单测：`reopen_queue_external_reopen_does_not_wait_without_driver`、
  `reopen_queue_external_reopen_joins_existing_driver`、
  `reopen_queue_read_retry_budget_is_per_request_and_merged`、
  `reopen_queue_zero_budget_does_not_park`。
- `fkv_shard/tests/cloud_reopen.rs`（12 passed）：
  - 改写 `cloud_read_waits_for_late_manifest` / `cloud_reads_share_driver_and_all_wake`
    （`pending_time=300ms`，driver 单次尝试需晚于 manifest 出现）；
  - `cloud_reads_fail_together_after_single_driver_attempt`（10 轮预算也只在约
    一轮后失败，证明 driver 不再多轮空转）；
  - 新增 `cloud_external_reopen_missing_manifest_fails_fast`、
    `cloud_external_reopen_joins_read_driver`、
    `cloud_pending_read_does_not_block_other_table`、`cloud_detach_fails_pending_read`。
- `fkv_shard/tests/cancel.rs`：`auto_reopen_pending_time_us` 调大到 300ms，使
  「挂起期间被调用方取消」仍在 driver 到期前可观察。
- `fkv_runtime/tests/auto_reopen.rs`：改写为
  `runtime_reopen_missing_manifest_fails_fast`（不再等待 50×1s）与
  `runtime_reopen_succeeds_after_manifest_appears`（调用方自行退避重试）；
  删除依赖旧 park 语义的 pending/waiter/detach 用例（同语义由 shard 的
  `StoreMode::Cloud` 用例覆盖）。

### 行为变化（已按方案 A 决策）

- 外部 `Reopen` 失败立即回 `ResourceMissing`，调用方需自行退避；不再由 shard
  挂满 `retry_times × pending_time`。
- 读请求最坏等一个 `pending_time`；driver 失败即失败，不再享受 driver 多轮
  重试。`auto_reopen_retry_times` 现为「每读请求轮数」（与 C++ 同名同义）。

### 残留（受本次 lane 文件范围限制）

- `doc/architecture/05-persistence-and-recovery.md` 与 `fkv_config` 的
  `auto_reopen_*` 字段注释/INI 文档登记未做（仅允许改 fkv_shard /
  fkv_runtime / fkv_task_reopen / fkv_standby 及测试）。代码注释已写明与
  C++ 同名同义，后续文档维护者需补登记。

## 复审（2026-11-05 v）

通过，归档。方案 A（严格对齐 C++）落地属实，亲读：

- `fkv_shard/src/lib.rs`：`Envelope.reopen_retry` :200 + `take_reopen_retry`
  :211-218（每请求预算，首次按 `auto_reopen_retry_times` 播种）；
  `park` :1121-1144 依次处理 `is_external_reopen` :1176-1179——外部 `Reopen`
  无同表条目时经 `merge_waiter` :1150 返回 `Some` 且不建 driver（:1130-1133），
  已有 driver 时合并为 waiter；`run_due_reopen` :1187-1216 到期 `handle` 一次，
  `Err` 即 `entry.fail(err)` :1213；`PendingReopen::fail` :1048-1056 全部 waiter
  同错、不重排。
- 决策登记（特别核对项）：`doc/architecture/05-persistence-and-recovery.md:57-61`
  写明「单次尝试 + 每读请求预算 + 外部 Reopen 不 park（对齐 C++）」；
  `fkv_config/src/lib.rs:306-311` 两字段注释标注与 C++ 同名同义。方案 A 决策
  已入文档/配置注释。
- 测试：`fkv_shard/src/lib.rs` 单测 :3611/:3631/:3651/:3682；
  `fkv_shard/tests/cloud_reopen.rs` 13 个 `#[compio::test]`，含
  `cloud_reads_fail_together_after_single_driver_attempt` :169（10 轮×100ms 下
  <600ms 证单轮）、`cloud_external_reopen_missing_manifest_fails_fast` :205、
  `cloud_external_reopen_joins_read_driver` :227；`fkv_runtime/tests/auto_reopen.rs`
  :64/:88。
- 备注：DONE 记 cloud_reopen「12 passed」，当前文件为 13 个 compio 测试（疑为
  后续轮次新增），不影响结论；未运行 cargo（约束）。
