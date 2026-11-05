# eng-39 reopen 无 driver/waiter 去重，每请求各自 replay

## 位置
- Rust: `crate/engine/fkv_shard/src/lib.rs:228-245`（`ensure_table_ready`：首次访问某表 `store.reopen(tbl).await`，`ResourceMissing` 忽略）、`:398-408`（`Request::Reopen` 调 `auto_reopen_with_sleeper`）、`crate/engine/fkv_task_reopen/src/lib.rs:87-115`（`auto_reopen_with_sleeper` 重试 `ResourceMissing`）。
- C++: `include/storage/shard.h:335-362`（`PendingReopenState`：driver + `waiters_`）、`src/storage/shard.cpp:303-328`（`EnqueueReopenWaiter`）、`:1070-1089`（`Reopen` 请求成为 driver）、`:1384-1395`（`OnTaskFinished` 遇 `ResourceMissing` + Cloud/StandbyReplica 重新作为 waiter）、`:382-401`（`CompleteReopenWaiters`）。

## 现状
Rust 每个请求首次访问某表时各自触发一次 `reopen`（`ensure_table_ready`），`ResourceMissing` 直接忽略等下次。`Request::Reopen` 是独立请求，用 `auto_reopen_with_sleeper` 重试。没有“多个请求共享同一个 reopen driver”的概念，也没有 waiter 列表。

## C++ 实现
C++ 分区级 `pending_reopens_`：第一个需要 reopen 的请求成为 driver（内部或外部），后续请求 `EnqueueReopenWaiter` 挂到同一 driver 的 `waiters_`。driver 完成（或失败）后 `CompleteReopenWaiters` 把 waiter 重新提交（成功）或全部以错误完成（失败）。`ResourceMissing` 在 Cloud/StandbyReplica 下自动重排为 waiter 重试。一个分区同一时刻只有一个 reopen 在跑，其余请求park。

## 差异与风险
- 并发下重复 replay：若未来并发化（eng-32），多个请求同时首次访问未加载的表，会各自 `reopen`，重复读 manifest、重复建 mapping，浪费 IO 且可能相互干扰（Rust 当前串行下被掩盖）。
- 无失败传播：C++ driver 失败时所有 waiter 一起失败；Rust 每个请求各自处理 `ResourceMissing`（忽略），一个请求失败不影响其它，语义不同。
- 无延迟 reopen 驱动：C++ `EnqueueDelayedReopenRequest` 对 `pending_time>0` 的 driver 延迟执行、`PromoteReadyDelayedReopenRequests` 按时间提升；Rust `auto_reopen_with_sleeper` 是同步重试循环，没有 delayed 队列与按时间提升。
- reopen 请求不能被 park 为 waiter（C++ `EnqueueReopenWaiter` `CHECK` 拒绝 `Reopen` 类型，改为 `InvalidArgs`）；Rust 无此约束，`Reopen` 与普通请求走同一 `ensure_table_ready` 路径。

## 对齐建议
- 引入 `pending_reopens: HashMap<TableIdent, PendingReopenState>`（driver + waiters），首个需要 reopen 的请求建 driver，其余挂 waiter。
- driver 完成/失败后 `CompleteReopenWaiters`：成功则重提交 waiter，失败则全部以错误完成。
- `ResourceMissing` + Cloud/StandbyReplica 自动重排为 waiter（对齐 `OnTaskFinished` 的 `auto_reopen`）。
- 实现 delayed reopen 队列 + 按 `execute_at_us` 提升（对齐 `delayed_requests_`/`PromoteReadyDelayedReopenRequests`）。

## 性能优化建议
- driver/waiter 去重避免并发下重复 manifest replay，是云模式 reopen 吞吐的关键。
- delayed reopen 让云 manifest 延迟出现时不阻塞其它分区。

## 严重度
P2（功能缺失 / 并发下行为不一致；当前串行下未触发）

## 审查结论（2026-11-04）

### 总结论

**部分采纳，严重度上调 P2 → P1。**

- C++ 侧 `pending_reopens_` / driver / waiter / delayed reopen / `Reopen` 拒绝入队的对照**全部属实**。
- Rust 侧「无 driver/waiter、`Reopen` 独立重试」属实。
- 但「当前串行下未触发」只对 **并发重复 replay** 成立。串行下已有两个真实问题：
  1. `Reopen` 请求首次到达时先经 `ensure_table_ready` 读一次 manifest，再在 `Request::Reopen` 分支读第二次，白白 replay 两遍。
  2. `auto_reopen_with_sleeper` 在 shard 任务内 `sleep` 重试；默认 `auto_reopen_retry_times = 10`、`auto_reopen_pending_time_us = 10_000_000`，单次 `Reopen` 最坏**阻塞整个 shard 线程 100 秒**，队列里其它 table 的请求全部排队。
- driver/waiter 去重本身**必须等 eng-32**（当前一个 shard 同时只有一个在途请求，waiters 永远为空，先写就是死代码）。
- Cloud/Standby 的 `ResourceMissing` 自动重排依赖运行模式，而 Rust 目前**没有 StoreMode 概念**（见 `issue/new/12-runtime-modes-api.md`），这一半也应暂缓。

因此建议拆成「现在可做」与「依赖 eng-32 / 12」两段，而不是把整条直接排期。

### 逐条核验表

| 论断 | 源码证据 | 判定 |
| --- | --- | --- |
| `ensure_table_ready` 每请求各自触发一次 `reopen` | `crate/engine/fkv_shard/src/lib.rs:279-298`：入口先查 `opened_table_set`（`:280-282`），命中直接返回；未命中才 `store.reopen`（`:286`）并把表插入集合（`:296`）。每个表在每次 attach 后只 reopen 一次 | **部分成立**。不是「每请求」，是「每表首次访问一次」；`AttachShard` 清空集合（`:576`）、`DetachShard` 驱逐（`:588-591`）后才会再次触发 |
| `ResourceMissing` 被忽略，等下次 | `:287-289` `Ok(()) \| Err(KvError::ResourceMissing) => {}` | 成立。且随后仍把表标记为 opened（`:296`），后续普通请求不会再尝试 reopen |
| `Request::Reopen` 是独立请求，用 `auto_reopen_with_sleeper` 重试 | `:478-488` 调 `store.auto_reopen_with_sleeper`；`fkv_task_reopen/src/lib.rs:109-137`、`fkv_store/src/lib.rs:1221-1238` 为两份同构重试循环 | 成立。**补充缺陷**：`handle` 在 match 前已统一调用 `ensure_table_ready`（`:366-368`），首次 `Reopen` 会先读一次 manifest（`:286`），再由 `:485` 读第二次。原文未提及 |
| 无 driver/waiter 去重 | 全仓无 `pending_reopens` / waiter 结构；`Envelope` 的 `reply` oneshot 由单个 `handle(...).await` 全程持有（`:177-228`） | 成立 |
| C++ `PendingReopenState` = driver + `waiters_` | `eloqstore 82b774c` `include/storage/shard.h:335-362`；`pending_reopens_` 定义在 `:362` | 成立（行号精确） |
| C++ `EnqueueReopenWaiter` / `CompleteReopenWaiters` / 延迟队列 | `src/storage/shard.cpp:303-328`（入队 + 建内部 driver）、`:330-350`（`EnqueueDelayedReopenRequest`）、`:352-380`（`PromoteReadyDelayedReopenRequests`）、`:382-401`（成功重提交 / 失败 `SetDone(err)`） | 成立（行号精确） |
| C++ `Reopen` 请求成为 driver | `:1070-1089`，外部 `ReopenRequest` 通过 `SetExternalDriver` 成为 driver | 成立 |
| C++ 普通请求遇 `ResourceMissing` + Cloud/Standby 变 waiter | `:1384-1395`：`err == ResourceMissing && req->AutoReopenRetry() && reopen_retry_remaining_ > 0 && (Cloud \|\| StandbyReplica)` → `auto_reopen`；`:1494-1497` → `EnqueueReopenWaiter(req)` | 成立。且只有 `ReadRequest` / `FloorRequest` / `ScanRequest` 覆写 `AutoReopenRetry() == true`（`include/eloq_store.h:141,218,272,300`），写请求不自动重排；原文未写这一限定 |
| C++ `Reopen` 不能被 park 为 waiter | `shard.cpp:593-604`：`Reopen` 类型直接 `SetDone(InvalidArgs)`；`EnqueueReopenWaiter` 首行 `CHECK(req->Type() != RequestType::Reopen)`（`:305`） | 成立 |
| Rust 无失败传播 | 成立。每个请求独立 `handle`，无共享 driver，失败只回给本请求；C++ driver 失败会 `CompleteReopenWaiters(..., err)` 连坐所有 waiter | 成立（但仅在并发/waiter 存在时才有意义） |
| Rust 无 delayed reopen，`auto_reopen_with_sleeper` 同步重试 | `fkv_shard/src/lib.rs:484-486` → `fkv_store/src/lib.rs:1227-1237`：循环 `sleep(pending_time)`。默认值见 `fkv_config/src/lib.rs:152-154`（10 次 / 10 s） | 成立，且**当前已可触发**：`Reopen` 请求最多阻塞 shard 100 s，不是 eng-32 之后才出现的问题 |
| 「当前串行下未触发」 | shard 主循环 `:177-228` 单请求串行；但单请求内的 `sleep` 阻塞与 `Reopen` 双读在串行下同样发生 | **部分成立**。重复并发 replay 被掩盖；阻塞与重复 replay 未被掩盖 |
| 行号 | Rust `ensure_table_ready` 原文 `:228-245` 实为 `:279-298`；`Request::Reopen` 原文 `:398-408` 实为 `:478-488`；`fkv_task_reopen` 原文 `:87-115` 实为 `:109-137` | Rust 行号已过期（eng-56 等改动导致）；C++ 行号准确 |
| 关联依赖 | `issue/new/eng-32-workloop-serialization.md`（并发化）；`issue/new/12-runtime-modes-api.md`（Cloud/Standby 模式未暴露）；`fkv_runtime::RuntimeConfig`（`:33-49`）与 `fkv_shard::ShardConfig`（`:22-56`）均无 mode 字段 | 成立。原文只提 eng-32，漏了运行模式依赖 |

### 开发计划（部分采纳）

#### 阶段 A：现在可做（不依赖 eng-32）

目标：消除当前串行下的阻塞与重复 replay，同时把 delayed reopen 的数据结构铺好，阶段 B 直接复用。

1. **消除 `Reopen` 双读**
   - `handle` 的 `ensure_table_ready` 统一入口（`:366-368`）排除 `Request::Reopen`。
   - `Request::Reopen` 分支自己负责一次 `reopen` + 重试；成功/失败都不要再依赖 `opened_table_set`。
   - 或者把 `ensure_table_ready` 改为返回 `AlreadyOpened`，`Reopen` 分支据此跳过首次读。
   - 验收：failpoint/计数断言首次 `Reopen` 只读一次 manifest（可用 `fkv_io` 的读计数或测试用 `ManifestIo` wrapper）。

2. **delayed reopen 队列替换 sleep 阻塞**
   - `ShardState` 增加 `delayed_reopen_li: BinaryHeap<Reverse<DelayedEntry>>`，`DelayedEntry { tbl, execute_at, retry_left, envelope }`。`Envelope` 的 oneshot 继续挂起，调用方语义不变。
   - 主循环从 `rx.recv().await` 改为 `select`（crossfire 接收 future + `compio::time::sleep_until(最早 deadline)`）；到期后把 entry 重新交给 `handle` 尝试 reopen，仍 `ResourceMissing` 且 `retry_left > 0` 则重新入堆，否则完成 oneshot。
   - `DetachShard` / `Shutdown` 必须先把队列中所有 entry 以 `NotRunning` 完成，再退出（对齐 C++ `shard.cpp:265-269` 的清理）。
   - 验收：
     - 现有 `crate/engine/fkv_runtime/tests/auto_reopen.rs::runtime_reopen_waits_for_manifest_from_another_writer`、`runtime_reopen_exhausts_retries` 保持通过（调用方仍等到成功/耗尽）。
     - 新增测试：`Reopen` 一个缺失表（长 `pending_time`）期间，对另一个已加载表发 `Get`，断言 `Get` 在远小于 `pending_time` 的时间内返回。
     - 新增测试：`DetachShard` / `Shutdown` 时挂起的 `Reopen` 以 `NotRunning` 返回，无泄漏、无 panic。

3. **去重重试实现（顺手）**
   - `Store::auto_reopen_with_sleeper`（`fkv_store/src/lib.rs:1221-1238`）与 `fkv_task_reopen::auto_reopen_with_sleeper`（`:109-137`）逻辑重复；阶段 A 改为前者只保留「打开 store 状态」的钩子，重试策略统一由 shard 的 delayed 队列驱动，避免两套重试语义漂移。

#### 阶段 B：依赖 eng-32（并发化之后）

目标：对齐 C++ `pending_reopens_`。**在 eng-32 落地前不做**，否则 waiter 永远为空。

1. `ShardState` 增加 `pending_reopen_map: HashMap<TableIdent, PendingReopenState>`：
   ```rust
   struct PendingReopenState {
     driver: ReopenDriver,          // Internal | External { reply }
     waiter_li: Vec<Envelope>,
   }
   ```
   对齐 C++ 的 `std::variant<ReopenRequest, ReopenRequest *>` 外部/内部 driver 区分。
2. 首个需要 reopen 的请求成为 driver；后续同表请求 `waiter_li.push`，不再各自读 manifest。
3. driver 完成：成功则逐个重新投递 waiter；失败则全部 `SetDone(err)`（对齐 `CompleteReopenWaiters`）。
4. 外部 `Reopen` 请求成为 driver；若同表已有 driver，则 `Reopen` 请求返回 `InvalidArgs`，禁止 park（对齐 `shard.cpp:593-604`）。
5. `DetachShard` / `Shutdown` 清理 map 并失败所有 waiter（已有阶段 A 的清理框架可扩展）。

#### 阶段 C：依赖 `12-runtime-modes-api`（Cloud/Standby）

1. `RuntimeConfig` / `ShardConfig` 增加 `StoreMode`，Runtime 透传到 shard。
2. 仅对 `Get` / `GetLarge` / `Floor` / `Scan`（对齐 C++ `AutoReopenRetry`）在 `Cloud` / `StandbyReplica` 下把 `ResourceMissing` 转成 delayed waiter 重试；写请求不自动重排。
3. 修改 `ensure_table_ready`：Cloud/Standby 下 `ResourceMissing` **不得**插入 `opened_table_set`，否则 manifest 后续可见也永远不会再读（`:289/:296`）。Local 分支保持现有懒创建语义。

### 风险

1. **eng-32 强依赖**：阶段 B 的 driver/waiter 在没有并发时不可观测，先实现会变成无法测试的死代码。必须跟 eng-32 同一批或紧随其后。
2. **运行模式缺失**：阶段 C 依赖 `12-runtime-modes-api`，当前 `fkv_runtime` 无 `StoreMode`；在模式落地前不要伪造 Cloud 分支，否则无法验收。
3. **主循环改造面大**：阶段 A 把 `rx.recv().await` 改成 deadline-aware `select`，会触碰 `Envelope` 所有权、OOM 重试、shutdown 排空。必须补 detach/shutdown 挂起请求的测试，避免 oneshot 悬挂。
4. **语义回归**：`ensure_table_ready` 当前对 `ResourceMissing` 的「空表」处理是 Local 懒创建的既定语义（`:287-289` 注释）。阶段 C 只能按 mode 分支，不能全局改成报错。
5. **重试节奏一致性**：C++ 是「延迟入队 + 按 `execute_at_us` 提升」，首次尝试并非立即；Rust 现在是「立即尝试 + N 次 sleep」。阶段 A 要明确保持「立即尝试一次，再等 `pending_time`」以免改变现有测试时序（`auto_reopen.rs` 断言 `elapsed >= 20ms`）。
6. **caller 取消**：挂起 `Reopen` 的 oneshot 若调用方提前 drop，delayed entry 仍持有 `Envelope`；出队重试时 `reply.send` 失败要静默丢弃，不能 panic。
7. **索引不同步**：本次只移动文件，未改 `issue/new/00-index.md`；该索引仍把 `eng-39` 列在TODO，需由索引维护者在合并时同步。

## 进展（2026-11-04，部分完成）

- 修复首次 `Reopen` 双读：main branch 下 `handle` 不再先
  `ensure_table_ready`（内部会 `reopen`）再进入 `Request::Reopen` 的
  `auto_reopen_with_sleeper`。branch 模式仍走 ensure 以建立 file 上限。
- 验证：`cargo nextest run -p fkv_shard -p fkv_runtime -p fkv_store`
  91 passed（含 auto_reopen / branch_attach / long_stress）。
- 未做：driver/waiter 去重、delayed reopen 队列；依赖 `eng-32` 并发模型。

## DONE（2026-11-04）

### 落地内容

1. **阶段 A1（`Reopen` 双读）**：源码已有 `skip_ensure`（`handle` 中 branch=main 时
   `Reopen` 不再先走 `ensure_table_ready` 再读一次 manifest），本次复核确认保留。
2. **阶段 A2（delayed reopen 队列替换 sleep 阻塞）**：新增 `ReopenQueue` /
   `PendingReopen`，shard 主循环从 `while let Ok(env) = rx.recv().await` 改为
   「先跑到期重试 → 有挂起条目时 `compio::time::timeout(剩余时长, rx.recv())`」。
   - `handle` 的 `Reopen` 分支改为单次 `store.reopen`，不再在请求任务内 `sleep`
     （旧实现最坏阻塞整个 shard 线程 `retry_times × pending_time`，默认 10 × 10s）。
   - 首个失败请求成为 driver，同表后续 `Reopen` 只挂 `waiter_li`
     （对齐 C++ `EnqueueReopenWaiter`），driver 完成/失败时统一通知全部 waiter，
     不再重复 replay manifest。
   - `retry_times == 0` 保持原语义：立即回 `ResourceMissing`（`park` 交回回执）。
   - detach / shutdown 时挂起条目统一以 `NotRunning` 结束（对齐 C++ Stop 先清
     `pending_reopens_`）；调用方已取消时 oneshot 发送失败静默丢弃，不 panic。
   - 主循环用 `saturating_duration_since + timeout`，避免 `timeout_at` 在
     `deadline - now` 上 panic；剩余 0 时长即刻到期回顶部重试，每轮都有一次尝试。

### 改动文件

- `crate/engine/fkv_shard/src/lib.rs`：`ReopenQueue` / `PendingReopen` /
  `run_due_reopen` + 主循环；`ShardState` 去掉 `auto_reopen_options`（策略归队列）。
- `crate/engine/fkv_runtime/tests/auto_reopen.rs`：3 个新集成测试。

### 测试

- `cargo nextest run --no-fail-fast -p fkv_shard -p fkv_runtime -p fkv_task_reopen`：
  35 passed，1 skipped。
- 新增用例：
  - `runtime_pending_reopen_does_not_block_other_tables`：`pending_time=1s` 的缺失表
    Reopen 挂起期间，另一表 `Get` 在 500ms 内返回；并断言该 Reopen 确实等满
    `pending_time` 才重试（防退化成「立即失败」的假通过）。
  - `runtime_reopen_waiters_complete_together`：同表两个并发 `Reopen`（driver + waiter）
    在 manifest 出现后同时成功。
  - `runtime_detach_fails_pending_reopen`：detach 后挂起请求立即回 `NotRunning`。
- `cargo clippy -p fkv_shard -p fkv_runtime --no-deps -- -D warnings`：0 警告。

### 未落地（拆分为子 issue）

- `issue/todo/eng-39a`：driver/waiter 的外部 driver 与「普通请求挂 waiter」路径（依赖 eng-32）。
- `issue/todo/eng-39b`：`fkv_store::auto_reopen_with_sleeper` 已无调用者，删除属 A3（热点文件冲突）。
- `issue/todo/eng-39c`：Cloud/StandbyReplica 下 `ResourceMissing` 自动重排（依赖 12-runtime-modes-api）。

### flake 记录

`fkv_runtime::ttl_cleanup::runtime_clean_expired_deletes_due_entries` 在高负载（并发会话编译中）
时偶发失败：用例用 `expire_ts = now+50ms`，机器忙时 put/get 超过 50ms 即被 TTL 过滤，
属固有竞态；单独重跑与多次整体重跑均通过，与本次改动无关。

## 复审（2026-11-05 r6b）

通过。

- `PendingReopen { driver, waiter_li }`（`crate/engine/fkv_shard/src/lib.rs:913-923`）、`ReopenQueue`（`:949`）、同表挂 waiter 不重复 replay（`:971-988`）、driver 成功重排 waiter / 失败同错完成（`:927-946`）；主循环 `run_due_reopen`（`:572,632`）替换 sleep 阻塞。
- 测试断言覆盖修复点：`crate/engine/fkv_runtime/tests/auto_reopen.rs:115` `runtime_pending_reopen_does_not_block_other_tables`——断言 Reopen 真挂起（`>=900ms`，`:147-150`）期间另一表 Get `<500ms` 返回（`:154-157`），防「立即失败」假通过；`:167` `runtime_reopen_waiters_complete_together`（driver+waiter 同成）；`:221` `runtime_detach_fails_pending_reopen`（detach 回 `NotRunning`）。
- 拆分件 `eng-39a/39b/39c` 均已在 `issue/reviewed/`。
