# rev-38 Cloud reopen 不触发 per-table 预热，attach 首轮之后新出现的分区永不预热

## 位置
- Rust: `crate/engine/fkv_runtime/src/lib.rs:1216-1231`（prewarm 仅在 start 时按 KvOptions 装配）、`:1299-1330`（`attach_shard` 是唯一 `crond.signal()` 触发点）、`:459-505`（`PrewarmState::plan`：只选 `attached && !done` 的 shard）；`crate/engine/fkv_shard/src/lib.rs:1601-1630`（`Reopen` / `ReopenExternal` dispatch 无任何 prewarm 动作）
- C++: `src/tasks/reopen_task.cpp:116-122`（`ReopenTask::Reopen`：Cloud + `prewarm_cloud_cache` 时 `prewarm_service->Prewarm(ThdDsShard(), tbl_id)`）、`include/tasks/prewarm_task.h:113-124`（`Prewarm` per-table / `PrewarmShard` per-shard 两个入口，注释标明 AttachShard 用后者、ReopenTask 用前者）

## 现状
fkv 的预热触发源只有一个：`Runtime::attach_shard` 成功后 `crond.signal()`。crond 一轮
= 列举远端分区 → 按 shard 分组 → 只给「已 attach 且 `done == false`」的 shard 预热全部
表；成功的 shard 置 `done`，后续 attach 同身份不再重跑。`Store::reopen` / `install_external_snapshot`
以及 shard 层的 `Request::Reopen` / `ReopenExternal` 都不产生任何预热信号。

后果：
1. attach 时刻远端还不存在、之后才被别的 owner 创建/推送的分区，本进程首次 Reopen 打开
   它时不会预热——首个读者逐页同步回源（读放大 + 尾延迟）。
2. `ReopenExternal`（强制采纳远端快照）之后也不预热，与 C++ 语义差异相同。

## C++ 实现
`ReopenTask::Reopen` 末尾（reopen_task.cpp:116-122）：Cloud 模式且 `prewarm_cloud_cache`
打开时，对刚 reopen 的分区调用 `PrewarmService::Prewarm(ds_shard_id, tbl_id)` 排队一轮
per-table 预热（PrewarmService 自己的后台 loop 消费，不阻塞请求）。即 attach 预热整
shard + 每次 reopen 预热单分区，两个触发源。

## 差异与风险
- 只影响 Cloud 模式 cache 命中率与冷读延迟（功能正确性不受影响：`read_page` miss 会
  同步回源），但预热是 prewarm_cloud_cache 用户显式打开的特性，reopen 是最常见的
  "新分区出现"事件，漏掉它使该特性在长生命周期 runtime 上只覆盖启动瞬间的分区集合。
- `PrewarmState` 已有按 shard 的 `done` 标记，缺少的是「per-table 已预热」粒度与
  reopen 信号源。

## 对齐建议
在 shard 的 `Reopen` / `ReopenExternal` 成功路径上（或 `Runtime::reopen` 便捷方法层）
对 Cloud 模式补一个 per-table 预热信号。实现建议：`PrewarmState` 增加
`pending_tables: VecDeque<TableIdent>`（容量上限对齐 C++ `kMaxPrewarmPendingFiles`
的有界队列思路，重复表去重），crond `tick` 先消费 pending 再处理 attach 触发的整 shard
轮；`Signal` 复用现有容量 1 通道。注意预热在 runtime 线程外，不能同步下载。

## 严重度
P2

## 核验记录
- 读 `fkv_runtime/src/lib.rs`：prewarm 装配（1216-1231）、`attach_shard`（1299-1316，
  signal 唯一调用点在 attach 成功分支）、`detach_shard`（1318-1330）、
  `PrewarmState::{attach,detach,plan}`（444-505）、`PrewarmCrond::run/tick/signal`
  （639-709）。
- 读 `fkv_shard/src/lib.rs`：`Request::Reopen` / `ReopenExternal` dispatch
  （1601-1630）确认无 prewarm/信号。
- 读 C++ `reopen_task.cpp:116-122`、`prewarm_task.h:104-137`（Prewarm/PrewarmShard
  注释），确认两触发源。

## 甄别（2026-11-05 r6）

采纳。逐项核实，行号为当日快照（fkv_runtime/fkv_shard 并行热改，意见原引行号已漂移）：

- Rust 属实。`fkv_runtime/src/lib.rs`：`PrewarmState`（~:441-514）仅 per-shard
  `attached/ident_li/done`，`plan` 只收 `attached && !done` 的 shard，无 per-table
  粒度；`PrewarmCrond::run`（~:644-664）阻塞在 `signal_rx.recv()`，无周期定时器；
  生产代码 `crond.signal()` 唯一调用点在 `attach_shard` 成功分支（~:1427-1429，
  其余 :1955+ 均在 tests）。`Runtime::reopen`（~:1675）/ `reopen_external`
  （~:1688）不触预热；`fkv_shard/src/lib.rs` `Request::Reopen`（~:2269-2277）/
  `ReopenExternal`（~:2279-2303）dispatch 无预热动作，全文件零 prewarm 引用
  （意见原引 :1601-1630 已漂移至 ~:2269）。⇒ 所有 shard 首轮 done 后不再有
  任何 tick；此后远端新建/推送的分区（含 auto-reopen 打开）永不预热，与意见一致。
- C++ 属实。`src/tasks/reopen_task.cpp:116-122`：Cloud + `prewarm_cloud_cache`
  时 `prewarm_service->Prewarm(ThdDsShard(), tbl_id)`（per-table）；
  `src/eloq_store.cpp:1053-1055`：AttachShard → `PrewarmShard`（per-shard）；
  `include/tasks/prewarm_task.h:113-124` 注释与两调用点吻合。C++ 双触发源、
  fkv 单触发源成立。

与相邻 issue 边界：
- cld-47a（reviewed，已做）只覆盖 attach→后台整 shard 触发与跨 shard 分发，
  **不含 reopen 触发**，本条不与之重复（意见对 cld-47a 覆盖范围的判断正确）。
- 12a（todo）做显式 `Request::Prewarm` 请求入口（外部 API 面）；本条是 reopen
  成功路径的自动信号（runtime 内部），互不替代。若 12a 先落地，本条可复用其
  per-table 预热执行路径。
- rev-34（todo）改 Cloud reopen 语义（Reopen 也走远端快照）；与本条同触
  `Request::Reopen` / `ReopenExternal` 成功路径，建议同批次接线，避免二次改 dispatch。

开发计划：
1. `PrewarmState` 增 per-table 有界待预热队列（对齐 C++ `kMaxPrewarmPendingFiles`
   思路，重复表去重）；`Runtime::reopen` / `reopen_external` 成功后（或 shard
   dispatch 成功回执层）投递并 `signal()`；仅 Cloud + `prewarm_cloud_cache` 生效。
2. `PrewarmCrond::tick` 先消费 pending per-table 目标，再跑整 shard 轮；
   `Signal` 复用容量 1 通道；预热仍在 runtime 线程外异步执行。
3. 测试：shard done 后远端新增分区 → `reopen` 后 cache 命中 / `prewarm_report`
   增长；非 Cloud 模式不触发。

热点文件标注：`crate/engine/fkv_runtime/src/lib.rs`（并行热改 lane，
`PrewarmState` / `PrewarmCrond` / `attach_shard` / `reopen` 全在其内）、
`crate/engine/fkv_shard/src/lib.rs`（另一并行 lane，dispatch 匹配臂需协调）；
`fkv_task_prewarm::prewarm_tables_with_concurrency` 已具备 per-table 执行能力。

P2 认可：仅影响 `prewarm_cloud_cache` 显式开启时的冷读延迟与命中率，功能
正确性不受影响（read miss 会同步回源）。

## DONE（2026-11-05）

**状态：代码 + 测试闭环，issue 关闭。**

### 改动

`crate/engine/fkv_runtime/src/lib.rs`：

1. `PrewarmState` 增 `pending_tbl_li: VecDeque<TableIdent>`（上限
   `MAX_PENDING_PREWARM_TABLES = 1000`，对齐 `kMaxPrewarmPendingFiles` 的有界
   思路；重复表去重，超限丢最早一条）。
2. `plan` 改为 `&mut self`：先按 attach 轮收集 `attached && !done` 的 shard，
   再 drain pending 单表目标并按 `TableIdent::shard_index` 合并进同一
   `PrewarmTarget`（同 shard 的 attach 轮与 reopen 目标一次跑完、重复表只留一份；
   未 attach shard 的 pending 直接丢弃，等 attach 整轮覆盖）。
   **pending 不受 `done_li` 限制**——这是 attach 之后新出现分区能预热的关键。
3. `PrewarmCrond::tick` 先消费 pending 再跑 attach 轮：整轮远端列举失败时仍执行
   pending 目标（per-table 不依赖 bucket 列举）；列举失败时**不**置 `done_li`
   （整轮不完整），回执带 `listing_failed`。
4. `Runtime::signal_table_prewarm`：reopen 成功后 `enqueue_table + crond.signal()`，
   只入队并唤醒后台线程，不阻塞 reopen 回执；仅 Cloud +
   `prewarm_cloud_cache` 有 `prewarm_state/prewarm_crond` 时生效。
5. 接线：`Runtime::reopen` 成功、`Runtime::reopen_external` 返回
   `InstallOutcome::Installed` 后投递；`WaitingRemote` 无可预热分区不投递。
   对齐 C++ `ReopenTask::Reopen` → `PrewarmService::Prewarm(tbl)`。

### 测试

- `fkv_runtime/src/lib.rs` 单测（`mod prewarm_tests`，5 passed）：
  `pending_table_targets_ignore_done_and_dedupe`（done 后仍消费、去重、
  未 attach 丢弃）、`prewarm_crond_consumes_reopen_table_after_attach_done`
  （attach 轮完成后远端新增分区经 pending 预热，报告 `table_num=1` 且
  `pulled_file_num>=2`，`done_li` 不被破坏）。
- `fkv_runtime/tests/prewarm.rs` 新增 S3 端到端
  `s3_reopen_triggers_per_table_prewarm`（无 `EKV_S3_ENDPOINT` 时跳过）：
  空远端 attach → 远端新增分区 → `reopen` → 等待出现 `table_num=1` 且
  `pulled_file_num>=2` 的预热回执。
- 既有 attach 预热 / 身份 / 列举 term 用例全绿。

### 残留 / 边界

- shard 内部 auto-reopen driver 合成的 `Request::Reopen` 不经 runtime API，
  不触发 per-table prewarm；本次按 issue 允许的「runtime 便捷方法层」接线。
  若要覆盖内部 driver，需在 `ShardConfig` 加跨 crate 回调/通道（本次未做）。
- `reopen_archive`（fkv 扩展）未投递预热；C++ 的 tag reopen 会预热，若需要
  可复用本状态队列另行接线。

## 复审（2026-11-05 v）

通过，归档。代码 + 测试闭环属实，亲读：

- `fkv_runtime/src/lib.rs`：`MAX_PENDING_PREWARM_TABLES` :542；
  `PrewarmState.pending_tbl_li`/`pending_set` :557-564；`enqueue_table` :615-625
  （去重 + 上限丢最早）；`take_plan_input` :631-639、`plan_targets` :659-706
  （attach 轮只收 `attached && !done` :673，pending :678-683 **只要求 attached、
  不看 `done_li`**——这是 attach 后新分区能预热的关键）；`PrewarmCrond::tick`
  :886-935 先消费 pending（列举失败也消费 :895-899，且不置 `done_li` :911）。
- 接线：`signal_table_prewarm` :1634-1640；`Runtime::reopen` :2228-2239 成功
  `Response::Reopen` 后投递；`reopen_external` :2247-2260 仅
  `InstallOutcome::Installed` 投递（`WaitingRemote` 不投）。
- 测试：`fkv_runtime/src/lib.rs` `mod prewarm_tests` :2592
  `pending_table_targets_ignore_done_and_dedupe`、:2774
  `prewarm_crond_consumes_reopen_table_after_attach_done`；
  `fkv_runtime/tests/prewarm.rs:133` `s3_reopen_triggers_per_table_prewarm`。
- 备注：DONE 原文称「`plan` 改为 `&mut self`」，后续轮次已重构为
  `take_plan_input` + `plan_targets`（锁外分桶去重），行为（pending 不受
  `done_li` 限制、去重、有界、列举失败仍消费）保持一致，非回归。
- 边界（DONE 自述，未另立项）：shard 内部 auto-reopen driver 合成的
  `Request::Reopen` 不经 runtime API，不触发 per-table prewarm；`reopen_archive`
  未投递——均为显式范围外。
