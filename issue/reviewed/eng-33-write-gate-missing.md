# eng-33 写队列/WriteGate/max_write_concurrency 全部缺失

## 位置
- Rust: 设计见 `todo.md` §4.3.5（“每分区一个 FIFO 写队列 + 一个 async WriteGate；同一时刻只有一个写任务；max_write_concurrency 用 Semaphore 实现背压”）——**代码中不存在**。全仓 grep `WriteGate`/`Semaphore`/`max_write_concurrency` 在 `crate/engine` 无命中。实际写路径：`crate/engine/fkv_shard/src/lib.rs:298-306`（`Request::Put` 直接 `state.store.put(...)`）。
- C++: `include/storage/shard.h:304-333`（`PendingWriteQueue`）、`src/storage/shard.cpp:623-656`（`TryStartPendingWrite`）、`src/tasks/task_manager.cpp:29-56,121-153`（`max_write_concurrency_`、`batch_write_pool_`/`bg_write_pool_`）。

## 现状
Rust 没有任何 per-partition 写队列、WriteGate 或写并发信号量。写请求和其他请求一样在 shard 全局队列里排队，由 work loop 一次一个地处理（见 eng-32）。所谓“写序列化”是靠全局串行意外达成的，不是 per-partition FIFO。

## C++ 实现
C++ 每个分区一个 `PendingWriteQueue`（`pending_queues_`），写请求 `PushBack` 后由 `TryStartPendingWrite` 在 `running_==false` 时启动队首，`OnTaskFinished` 再启动下一个——**同一分区同时只有一个写任务**，但不同分区、以及写与读可并发。`max_write_concurrency_` 是全局写任务上限（`GetBatchWriteTask`/`GetBackgroundWrite` 超限返回 null，请求留在队列重试）。compact/file-gc/local-gc/clean-expired 作为内部请求嵌入同一队列，保证与用户写串行。

## 差异与风险
- 设计文档承诺的背压机制（Semaphore、排队请求不占任务槽）未实现；`queue_size` 是 shard 全局请求队列长度，不是写并发。
- 无 `max_write_concurrency`：C++ 可限制同时进行的写/后台写数量以控内存（每个 `BatchWriteTask` 持有 applying page、overflow、leaf_triple 等）；Rust 若未来并发化（eng-32），写任务峰值内存无上限。
- 内部请求（compact/gc/clean-expired）没有嵌入写队列的语义：Rust 里它们是普通 Request，与用户写在同一全局队列 FIFO，但因为没有 per-partition `running_` 门，无法保证“同一分区写与后台写严格串行”的粒度（目前靠全局串行兜底）。
- 文档与代码不符，误导后续维护者以为写并发已受控。

## 对齐建议
- 在 `ShardState` 增加 `pending_queues: HashMap<TableIdent, PendingWriteQueue>`，写类请求（Put/PutLarge/Truncate/Drop/Gc/Compact/CreateArchive/CreateBranch/DeleteBranch/CleanExpired）入队，读类（Get/GetLarge/Scan/Floor/Reopen）直接进 ready。
- `PendingWriteQueue` 带 `running_` 标志，`TryStartPendingWrite` 在 `running_==false` 且队非空时启动队首；`OnTaskFinished`（等价物）置 `running_=false` 并触发下一条。
- 增加全局 `max_write_concurrency`（`Semaphore` 或计数器），`GetBatchWriteTask` 超限返回 pending 重试。
- 若短期不并发化（保持 eng-32 串行），至少把 `todo.md` §4.3.5 标注为“未实现”，避免误读。

## 性能优化建议
- per-partition FIFO 让不同分区的写可并行（配合 eng-32 并发化），提升批量写吞吐。
- `max_write_concurrency` 作为内存背压，防写任务峰值 OOM（对齐 C++ `GetBatchWriteTask` 返回 null 的重试语义）。

## 严重度
P1（功能缺失 / 文档与代码不符）

## 审查结论（2026-11-04）

**总结论：部分采纳。** 缺失事实成立，但“全部缺失”的说法需修正：`max_write_concurrency`
在 `fkv_config` 里有字段与解析，只是**零消费**；真正缺的是 per-partition 写队列与背压接线。
本条与 `03-shard-request-concurrency.md`、`eng-32-workloop-serialization.md` 高度重叠，
且 eng-32 的正文已明确“写路径仍按 eng-33 的每分区 FIFO 串行”——**eng-33 是 eng-32 的前置条件，
不能脱离 eng-32 单独实现**（当前全局串行下，per-partition 队列无任何可观测行为差异）。

### 逐条核验

| # | 原文断言 | 证据 | 结论 |
|---|---|---|---|
| 1 | `crate/engine` 无 `WriteGate` / `Semaphore` | 全仓 `rg 'WriteGate\|Semaphore'` 在 `crate/` 0 命中 | 属实 |
| 2 | `max_write_concurrency` 全无 | 仅 `crate/foundation/fkv_config/src/lib.rs:42`（字段）、`:113`（默认 0）、`:283`（`set` 解析）；`ShardConfig` 未传递，`Shard` 未读取 | **措辞需修正**为“已定义未接线”，非“全无” |
| 3 | 无 per-partition 写队列 | `fkv_shard/src/lib.rs:93` 单一 `bounded_async::<Envelope>(config.queue_size)`；`:172` `while let Ok(envelope) = rx.recv().await` 串行处理；无 per-table 队列 | 属实 |
| 4 | `Request::Put` 直接 `store.put` | 现位于 `fkv_shard/src/lib.rs:371-380`，`state.store.put(&tbl, &batch).await?`（原文 `298-306` 行号已漂移） | 属实 |
| 5 | `queue_size` 是 shard 全局请求队列长度 | `lib.rs:42` 定义、`:89-93` 建 channel，不区分读写、不区分 table | 属实 |
| 6 | C++ `PendingWriteQueue` / `TryStartPendingWrite` / `max_write_concurrency_` / `batch_write_pool_` | `cpp/eloqstore/` 为**空目录**，`cpp/` 下仅 `build/`、`shim/`；`include/storage/shard.h`、`src/storage/shard.cpp`、`src/tasks/task_manager.cpp` 在本仓不可达 | **不可核验**。行号引用风格也与 `eng-56` 的 `eloqstore/src/...` 前缀不一致，需统一并注明外部检出 |
| 7 | `todo.md` §4.3.5 | `todo.md:307` 为 `### 4.3 关键 Rust 设计`，`:329-331` 是其下第 5 条列表项“写序列化”；无 4.3.5 小节 | 内容一字不差，**编号需改为 §4.3 第 5 条** |
| 8 | 与 03 / eng-32 / 20 重叠 | 03 建议 2“按 table 维护写队列：同一 table 写串行、不同 table 并行”；eng-32“写路径仍按 eng-33 的每分区 FIFO 串行”；20 是写合并（正交） | 重叠成立，本条是其**前置/拆分子项** |

### 采纳范围（收窄后）

1. 文档纠偏（立即、低成本）：`todo.md` §4.3 第 5 条标注“未实现”，避免维护者误读写并发已受控。
2. 背压接线：`max_write_concurrency` 从 `fkv_config` → `ShardConfig` → `Shard` 传递（当前完全断链，
   与 `eng-56` 的“配置漏传”同类），默认 0 的语义要显式定义（0 = 不限，或 0 = 按核数）。
3. per-partition `PendingWriteQueue` + `running_` 门：**并入 eng-32 实施**，作为其写路径串行化的落地形态；
   不要单独开一条只为“补一个当前无人观察的队列”的改动。
4. 内部请求入队语义（compact / file-gc / clean-expired 与用户写同队列串行）随 3 一起做，
   当前全局串行已隐式满足，无独立风险。

### 开发计划（并入 eng-32 后的增量）

- 阶段 A（独立可交付）：`ShardConfig` 增加 `max_write_concurrency`，`Shard::start` 透传；
  `todo.md` 标注实现状态。验收：配置项可从 `fkv_config` 读入并被 `Shard` 持有（单测断言）。
- 阶段 B（依赖 eng-32 的 ready 集）：`ShardState` 增 `pending_queues: HashMap<TableIdent, PendingWriteQueue>`；
  写类请求入队，`running_ == false` 时启动队首，完成回调置 `false` 并续队首。
  验收：同 table 两个 put 顺序执行，不同 table 的 put 可交错（日志/探针断言 in-flight 表名不同）。
- 阶段 C：全局 `Semaphore`/计数器限流，超限请求回 pending 不占任务槽。验收：并发写上限测试。

### 风险

- **空转风险**：若 eng-32 未落地就实现阶段 B，队列只是把全局 FIFO 换成 per-table FIFO，
  行为不变、代码量与调试成本净增。必须与 eng-32 捆绑。
- **屏障交互**：`AttachShard` / `DetachShard` / `Shutdown` 不能进写队列，否则可能与排空逻辑互等；
  需旁路或提升为屏障请求。
- **COW root 顺序**：写队列只保证同 table 写序，读若绕过队列，root 切换时读需看到稳定快照
  （与 `27-no-cow-read-snapshot`、`eng-41` 的 pin 语义耦合）。
- **饥饿**：内部 compact/gc 与用户写同队列 FIFO 时，长 compact 阻塞前台写；
  建议内部请求走队尾且允许前台插队，或单列低优先通道（对齐 eng-34 的优先级门）。
- **默认值陷阱**：`max_write_concurrency: 0` 若直接当信号量许可数，会变成 0 许可死锁，
  必须先定义 0 的语义。

## 实施记录（2026-11-04）

按任务要求收紧为「结构 + 配置接线 + 串行执行器正确性」，真并发留给后续。
上节「审查结论」中的阶段 A 与阶段 B 的结构部分已落地。

### 改动文件

- `crate/engine/fkv_shard/src/write_gate.rs`（新）：`PendingQueue` + `WriteGate`。
  per-partition FIFO、`running` 门（同分区单写）、全局在跑计数上限（`0` = 不限）、
  `ready_li` 就绪分区队列、`remove_pending` / `clear`（Drop / Detach / Shutdown 清队列）。
  5 个单测覆盖：同分区 FIFO 互斥、跨分区独立、上限阻塞后恢复、0 = 不限、清队列。
- `crate/engine/fkv_shard/src/lib.rs`：
  - 写类请求（Put/PutLarge/Truncate/Drop/Gc/Compact/CreateArchive/CreateBranch/
    DeleteBranch/CleanExpired）经 `WriteGate` 派发；`Reopen` / Attach / Detach /
    Shutdown 保持旁路（`Reopen` 走既有延迟重试队列，控制请求不得与排空互等）。
  - 原单请求处理抽为 `process_envelope`（OOM 重试、写后维护、回执语义不变）。
  - `drain_write_gate` / `run_gated_write`：同分区按 FIFO 续跑，跨分区按 ready 顺序推进；
    `Drop` 后丢弃该分区排队请求，`DetachShard` / `Shutdown` 清全部排队并回 `NotRunning`。
  - `ShardConfig::max_write_concurrency: usize`（0 = 不限）。
- `crate/engine/fkv_runtime/src/lib.rs`：`KvOption::max_write_concurrency` → `ShardConfig`。
- `crate/engine/fkv_shard/tests/write_gate.rs`（新）：同分区并发写全部落盘（并发限 1
  无死锁）、跨分区写互不阻塞；`read_batch.rs` / `oom_retry.rs` / `backend.rs` 补字段。
- `todo.md` §4.3 第 5 条标注实现状态，去掉「Semaphore 背压已实现」的错误暗示。

### 与 eng-34 的重叠关系

- **不重叠**：eng-34 管高/低优先 ready 集合与 yield 预算；本条管写请求的排队形态与
  全局写额度。两条只在「写请求进哪个队列」处相接。
- **约束**：eng-34 的低优先队列必须保持分区 FIFO，写任务应初始 High、完成前自降 Low，
  不能静态判 Low。该约束已写入 `eng-33a`。

### 测试结果

- `cargo check -p fkv_shard`：通过。
- `cargo nextest run -p fkv_shard`：13 passed（含新增 5 单测 + 2 集成测试）。
- `cargo nextest run -p fkv_shard -p fkv_runtime -p fkv_store`：116 run，114 passed，
  2 failed。失败为 `fkv_store::truncate_drop` 两条 `removed >= 1`，属另一位 agent 正在
  改的 `fkv_store/src/lib.rs`（gc 文件回收），与 `fkv_shard` 改动无调用关系。
- `cargo clippy -p fkv_shard --no-deps --all-features --all-targets -- -D warnings`：
  本条新增/修改代码零警告；剩余 3 条来自并发新增的 `backend.rs`（`derivable_impls`、
  `large_enum_variant`、`too_many_arguments`），非本 issue 文件。

### 残留

- 跨分区写真正并行：依赖 eng-32b（per-table 状态、`Store::put` 去 `&mut self`）→ `eng-33a`。
- 队列深度 / 在跑写指标 → `eng-33b`（并入 `eng-32c` 实施）。
- 优先级与 yield 预算 → `eng-34`。
- C++ 行号证据无法在本仓核验（`cpp/eloqstore/` 为空，需外部检出）。

## DONE（2026-11-04）

串行形态的 per-partition 写队列、`WriteGate`、`max_write_concurrency` 接线与正确性
测试已落地；真并发与指标已拆到 `issue/todo/eng-33a-*`、`issue/todo/eng-33b-*`。

## 复审（2026-11-05 r6）

结论：**通过**。串行形态 WriteGate + max_write_concurrency 接线属实。

- `crate/engine/fkv_shard/src/write_gate.rs:37-208`：`WriteGate`（per-partition FIFO `PendingQueue` + `running` 门 + 全局 `max_concurrency`，0=不限），单测 5 条（同分区互斥 :230、跨分区独立 :276、上限阻塞恢复 :294、0 不限 :313、清队列 :327）。
- `fkv_shard/src/lib.rs:146` `ShardConfig::max_write_concurrency`；`:573` `WriteGate::new(config.max_write_concurrency, ...)`；`fkv_runtime/src/lib.rs:1149` KvOption → ShardConfig 透传。
- 集成测试 `fkv_shard/tests/write_gate.rs` 存在；`todo.md:326` 已标注「写序列化（已落地）」。
- 残留（真并发 eng-33a、指标 eng-33b）均已按 md 记录分流；eng-33b 本轮亦通过（见其文件）。
