# eng-39c Cloud / StandbyReplica 模式下的自动 reopen 重排（阶段 C）

## 来源
`issue/done/eng-39-reopen-no-driver-waiter.md` 阶段 C。

## 待做
1. `RuntimeConfig` / `ShardConfig` 增加 `StoreMode`，Runtime 透传到 shard。
2. 仅 `Get` / `GetLarge` / `Floor` / `Scan` 在 `Cloud` / `StandbyReplica` 下把
   `ResourceMissing` 转成 delayed waiter 重试；写请求不自动重排
   （对齐 C++ `AutoReopenRetry()`，见 `include/eloq_store.h:141,218,272,300`）。
3. `ensure_table_ready` 在 Cloud/Standby 下 **不得** 把 `ResourceMissing` 的表
   `open_empty` 并插入已打开集合（当前 `crate/engine/fkv_shard/src/lib.rs:316-327` 对
   `ResourceMissing` 一律按空表懒创建），否则 manifest 后续可见也永远不会再读。

## 阻塞
依赖 `issue/new/12-runtime-modes-api.md`：当前 `fkv_runtime` 没有 `StoreMode` /
运行模式概念（`RuntimeConfig`、`ShardConfig` 均无 mode 字段）。模式落地前实现无法验收，
不应伪造 Cloud 分支。

复核（2026-11-04）：`fkv_types::StoreMode` 类型已存在（`fkv_standby` 在用，
`StandbyRole::from_mode`），但 `RuntimeConfig` / `ShardConfig` 仍未暴露 mode，
`issue/new/12-runtime-modes-api.md` 也未被认领；本条继续阻塞。

## 前置
`eng-39` 阶段 A2 已落地的 `ReopenQueue`（延迟队列 + waiter 去重）可直接复用作为
「读请求遇 `ResourceMissing` 挂 waiter」的载体。

## DONE（2026-11-04）

### 实现要点
1. **模式透传**：`ShardConfig.mode: StoreMode`；`fkv_runtime` 用 `KvOption::derive_mode()`
   派生（无 KvOption 时 `Local`）并随 `Backend` 一起下发。`StandbyMaster/Replica` 目前
   仍被 `backend_of` 显式拒绝（standby 执行器未接入），模式通路已就位。
2. **不做本地懒创建**：`ShardState.reopen_on_missing`（Cloud / StandbyReplica）为真时，
   `ensure_table_ready` 不再把 `ResourceMissing` 当空表 `open_empty`，而是交给上层；
   manifest 暂不可见不再被误判成空表，也不会「之后再可见也不会再读」。
3. **读请求挂等待重排**（对齐 C++ `AutoReopenRetry`）：
   - `PendingReopen` 改为 `driver: Option<回执>`（外部 driver = `Reopen` 请求；
     内部 driver = 读请求触发）+ `waiter_li: Vec<Envelope>`；
   - `process_envelope`（`GetLarge` / `Reopen`）与 `run_read_batch`（`Get` / `Scan` / `Floor`）
     遇 `ResourceMissing` 时把整条 envelope 挂进延迟队列，同表只跑一个 driver；
   - driver 的 `reopen` 成功后 waiter 经主循环 `wake_li` 重新投递（对齐
     `CompleteReopenWaiters`），失败或重试耗尽则 driver 与 waiter 同错；
   - 写请求不重排，直接以 `ResourceMissing` 失败（对齐 C++ 只有读请求覆写 `AutoReopenRetry`）。
4. Local / Mem 模式行为不变（懒创建 + 立即返回），由 `local_read_missing_table_is_empty_and_immediate`
   守住回归。

### 改动文件
- `crate/engine/fkv_shard/src/lib.rs`
- `crate/engine/fkv_shard/tests/cloud_reopen.rs`（新增，5 个用例）
- `crate/engine/fkv_shard/tests/{backend,read_batch,write_gate,index_cache_metrics,oom_retry}.rs`
  （`ShardConfig` 新增 `mode` 字段）
- `crate/engine/fkv_runtime/src/lib.rs`（mode 派生与透传）

### 验证
- `cargo nextest run -p fkv_shard --no-fail-fast`：20 passed
- `cargo nextest run -p fkv_runtime --no-fail-fast`：30 passed，1 skipped
- `cargo clippy -p fkv_shard -p fkv_runtime --no-deps --all-targets -- -D warnings`：0 警告
- 新用例：`cloud_read_waits_for_late_manifest`（挂等到 manifest 出现后重排并返回真实数据）、
  `cloud_reads_share_driver_and_all_wake`（4 个并发读挂同一 driver，一起唤醒）、
  `cloud_reads_fail_together_after_retries_exhausted`（driver + waiter 同错）、
  `cloud_write_missing_table_fails_without_lazy_create`（写不重排、不建空目录）、
  `local_read_missing_table_is_empty_and_immediate`（Local 语义不变）。

### 残余风险
- 测试用 `StoreMode::Cloud` + Local IO 后端在本地确定性复现「manifest 延迟可见」：模式只决定
  `ResourceMissing` 语义，与 IO 后端正交，机制层已被覆盖；**真实 S3 端到端未在本地跑**
  （`fkv_cloud/tests/s3_rustfs.rs` 需要 rustfs 环境），云上联调时建议复核一次。
- `StandbyReplica` 仍被 `Runtime` 拒绝启动，故当前只有 Cloud 生效；standby 执行器接入后
  同一条 `mode` 通路自动覆盖，无需再改 shard。
- 挂起读请求的首轮尝试会记一条 `ResourceMissing` 错误指标（重排前的尝试确实失败了），
  与既有 `Reopen` 挂起路径一致。

## 复审（2026-11-05 r6）

结论：**通过**。

- 模式透传：`ShardConfig.mode`（`fkv_shard/src/lib.rs:552` 消费为 `reopen_on_missing`）；`fkv_runtime` `derive_mode` 派生下发。
- 不做懒创建：`:873-882` `reopen_on_missing` 为真时 `ResourceMissing` 不 `open_empty`，交上层挂等。
- 挂等重排：`PendingReopen`（:900-928）+ `park`（:958-973）+ 主循环 `wake_li`（:570/:619/:635）；写请求不重排直接失败。
- 5 个测试齐备：`fkv_shard/tests/cloud_reopen.rs:106/:133/:161/:192/:221`，断言覆盖等待时延（:121-125）、共享唤醒（:150-153）、同错（:179-180）、Local 语义不变。
- 残余风险（真实 S3 端到端、StandbyReplica 通路）与 md 记录一致，归 12* 后续。
