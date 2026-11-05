# eng-39a reopen driver/waiter 并发模型与 delayed reopen 对齐（阶段 B）

## 来源
`issue/done/eng-39-reopen-no-driver-waiter.md` 阶段 B。阶段 A2（延迟队列 + 同表 waiter 去重）
已落地，本子项只做依赖并发模型的剩余部分，不复述已完成内容。

## 现状（可复用的基座）
`crate/engine/fkv_shard/src/lib.rs` 的 `ReopenQueue` / `PendingReopen` / `run_due_reopen` 已提供：
- 按 `due` 提升的延迟重试队列（主循环 `compio::time::timeout` 驱动，不阻塞 shard）；
- 同表 driver + `waiter_li` 去重（对齐 C++ `EnqueueReopenWaiter`）；
- detach / shutdown 清理（`fail_all(NotRunning)`）。

## 待做
1. `ShardState` 增加 `pending_reopen_map: HashMap<TableIdent, PendingReopenState>`，
   `driver` 区分 `Internal | External { reply }`（对齐 `include/storage/shard.h:335-362`）。
2. 外部 `Reopen` 成为 driver；同表已有 driver 时返回 `InvalidArgs`，禁止 park
   （对齐 `src/storage/shard.cpp:593-604`、`EnqueueReopenWaiter` 首行 `CHECK`）。
3. 普通读请求（`Get` / `GetLarge` / `Floor` / `Scan`）遇 `ResourceMissing` 挂成 waiter 重排
   （对齐 C++ `AutoReopenRetry`；写请求不自动重排，见 `include/eloq_store.h:141,218,272,300`）。
4. delayed reopen 按 `execute_at_us` 提升，首次尝试也走延迟（对齐
   `PromoteReadyDelayedReopenRequests`）。

## 阻塞
依赖 `issue/new/eng-32-workloop-serialization.md`：当前 shard 同时只有一个在途请求，
「外部 driver 冲突返回 `InvalidArgs`」与「普通请求挂 waiter」两条路径不可观测，
先实现即无法测试的死代码。

复核（2026-11-04）：`eng-32` 与子项 `eng-32a/b/c` 已在 `issue/todo/`，由其它会话推进中；
本条等并发模型落地后再接手，避免与其冲突。

## 验收
并发下同表多个请求只触发一次 manifest replay：用 manifest 读计数（`fkv_io` 读计数或
`ManifestIo` wrapper）断言 driver 期间 replay 次数为 1；driver 失败时全部 waiter 连坐失败。

## 复审（2026-11-04）：核心已落地，剩余子项分流

### 已落地
- **待做 1（同表 driver/waiter 去重）**：`ReopenQueue` / `PendingReopen` 已按表建 driver +
  `waiter_li`；driver 由 `park` 内 `entry_index_or_insert` 建立，外部 driver = `Some(回执)`、
  内部 driver = `None`，等价 C++ `PendingReopenState` 的 external / internal 区分。
- **待做 3（读请求遇 `ResourceMissing` 挂 waiter 重排）**：本轮随 Cloud/Standby 模式一起落地
  （原阻塞前提「eng-32 未并发」已被 eng-32a 读批 + eng-33 写门消除，同一时刻可有多条在途）。
  代码：`crate/engine/fkv_shard/src/lib.rs` 的 `is_auto_reopen_read`、`process_envelope` /
  `run_read_batch` 里的 park、`run_due_reopen(..., wake_li)` 重排；测试
  `crate/engine/fkv_shard/tests/cloud_reopen.rs`。

### 拒绝
- **待做 2（同表已有 driver 时外部 `Reopen` 返回 `InvalidArgs`）**：C++ 那条 `CHECK`
  是其「外部 driver 唯一」的内部状态机约束。fkv 把后续 `Reopen` 挂成 waiter，driver 成功后
  重排并回 `Response::Reopen`：等待方拿到成功而不是报错，语义更强，故不引入报错分支。
- **待做 4（delayed reopen「首次尝试也延迟」）**：按 `execute_at_us` 延迟首次尝试会让每次
  `Reopen` 至少等一个 `pending_time`（默认 10s），把「立即尝试」退化成长等待；`eng-39`
  阶段 A2 已明确保留「立即尝试一次 + 再等 `pending_time`」的时序，不采纳。

## DONE（2026-11-04）

driver/waiter 与读请求重排的实际实现与验证见 `issue/done/eng-39c-cloud-standby-auto-reopen-mode.md`
（本轮一并落地）：`PendingReopen.driver: Option<回执>` + `waiter_li: Vec<Envelope>` +
`wake_li` 重排；测试 `crate/engine/fkv_shard/tests/cloud_reopen.rs` 5 例全绿。

## 复审（2026-11-05 r6）

结论：**通过**（实现载体 eng-39c，拒绝项理由成立）。

- driver/waiter：`fkv_shard/src/lib.rs:900-910` `PendingReopen { driver: Option<oneshot::Sender>, waiter_li: Vec<Envelope> }`；`:915-919` `wake`（外部 driver 回 Reopen、waiter 交回主循环）；`:922-928` 失败连坐；`:958-973` 同表去重（首个 Reopen 成外部 driver，其余挂 waiter）。
- 读请求重排：`:552` `reopen_on_missing`（Cloud/StandbyReplica）；`:873-882` `ensure_table_ready` 在该模式下不再懒创建空表；主循环 `wake_li` 重排（:570/:619/:635）。
- 测试 `fkv_shard/tests/cloud_reopen.rs:106`（等待迟到 manifest 且真实返回数据 :119-125）、`:133`（4 并发读共享 driver 一起唤醒）、`:161`（重试耗尽 driver+waiter 同错 :179-180）。断言真实覆盖。
- 待做 2/4 的拒绝理由（更强语义 / 首试立即）与 md 记录一致。
