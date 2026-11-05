# 03 P0：shard 串行 await 每个请求，无法交错 IO

## 现象

`fkv_shard` 的主循环是：

```rust
// crate/engine/fkv_shard/src/lib.rs:154
while let Ok(envelope) = rx.recv().await {
  let result = handle(&mut state, envelope.req).await;
  ...
}
```

`handle` 会完整执行一次 put/get/scan，包括所有 page IO、manifest fsync、TTL 清理，期间不会从队列取下一条请求。

## C++ 对照

eloqstore 的 shard 有 task manager / pending write queue：

- 读任务可以并发在 shard 内推进
- 写任务按 partition 串行，但不同 partition 的 IO 可以交错
- 任务在 IO 点 yield，完成后由 task manager 继续推进

参考 `docs/architecture/04-execution-model.md`。

## 影响

- 一条慢 IO（尤其 scan 或 fsync）阻塞该 shard 的所有 table/请求
- io_uring 的并发深度基本为 1
- 多线程 `num_threads` 只能靠 shard 间并行，单表吞吐受限
- 与 C++ 的异步任务模型差距明显

## 建议

1. 把每个请求包成独立 compio task（`Runtime::spawn` / `spawn_local`），由运行时调度。
2. 按 table 维护写队列：
   - 同一 table 的写串行
   - 不同 table 的写可并行
   - 读不阻塞写队列，但需要和 COW root 切换做顺序保证
3. 保持 attach/detach/shutdown 的屏障语义。
4. 增加 in-flight 上限和队列深度指标。

## 验收

- 单 shard 同时发起 32 个 get，IO 能并发在飞
- 慢 table 的 scan 不阻塞其他 table 的 get
- attach/detach 与并发请求的线性化测试通过

## 审查结论（2026-11-04）

### 总结论

**与 `eng-32` 重复，不独立开发。** 主体（串行 work loop、C++ task manager 对照、spawn 并发化）100% 被 `eng-32-workloop-serialization.md` 覆盖，写队列部分是 `eng-33`、优先级/预算是 `eng-34`、并发化安全前置是 `eng-41`。`00-index.md:26,40` 也已标注「03 ≈ eng-32」。

判定为**部分采纳**：本体并入 `eng-32`，本文件只保留独有内容（验收用例、in-flight 上限与队列深度指标、并发下 attach/detach 线性化），作为 `eng-32` 的验收附录，**不单独排期、不单独实现**。

同时修正 3 处硬伤：

1. 文档路径笔误：应引 `doc/architecture/02-execution-model.md`（非 `docs/architecture/04-execution-model.md`）。
2. 建议 1「把每个请求包成独立 compio task」当前措辞不可直接落地，见下表第 4 条。
3. 严重度应为 **P1**（吞吐/尾延迟不达标，无语义错误），对齐 `eng-32`，非 P0。

### 逐条核验表

| 条目 | 判定 | 证据 / 去向 |
|---|---|---|
| 现象：`while let Ok(env) = rx.recv().await { handle(&mut state, env.req).await }` 串行 | 完全重复 `eng-32` | `crate/engine/fkv_shard/src/lib.rs:177-236`（`handle` 在 :201，回执在 :232，`shutdown` 才 break） |
| C++ 对照：task manager / pending write queue / 多协程 resume | 重复 `eng-32` + `eng-33` | `/Users/z/git/db/kv/eloqstore/src/storage/shard.cpp:115-212`（`WorkLoop` 批量 dequeue，idle 判据含 `task_mgr_.NumActive()==0`）、`:1280-1331`（`ExecuteReadyTasks` 循环 resume `ready_tasks_`）、`src/tasks/task_manager.cpp:123,140`（`max_write_concurrency_` 门）、`include/storage/shard.h:110-111`（高/低优先队列） |
| 影响：单 shard io_uring 深度 1、慢 IO 阻塞全 shard | 重复，且已入档 | `doc/architecture/02-execution-model.md:35-42` 明写「io_uring 并发深度实际为 1」并直指 `eng-32` |
| 建议 1：每请求 `Runtime::spawn` / `spawn_local` | 重复 `eng-32`，**且原措辞不可行** | `compio` 0.19.2 的 `spawn<F: Future + 'static>`（`compio-runtime-0.12.6/src/lib.rs:559`，无 `Send` 约束但要求 `'static`，非 runtime 上下文 panic）。`handle` 收 `&mut ShardState`（`fkv_shard/src/lib.rs:349`），且读路径也会 `ensure_table_ready` → `reopen`/`open_branch`/`set_term` 并改 `opened_table_set`（`:279-298, :367-369`）。`Store::put` 仍 `&mut self`（`fkv_store/src/lib.rs:817`），`get`/`scan`/`floor` 是 `&self`（`:406, :452, :2027`）。故必须先拆 per-table 打开状态（OnceCell/异步锁）+ 读共享 `&Store`，才能 spawn。 |
| 建议 2：按 table 写队列、同表串行、异表并行 | 完全重复 `eng-33` | `eng-33` 已给 `PendingWriteQueue`/`TryStartPendingWrite`/`max_write_concurrency` 的逐行对照 |
| 建议 3：保持 attach/detach/shutdown 屏障语义 | **部分独有** | `eng-41` 只讲 shutdown 排空与 fixed_buffer，`eng-57` 只讲 term CAS 分裂，均未定义「in-flight 请求与新 attach/detach 的线性化」。并发化后 `AttachShard` 改 `state.term/branch`、`DetachShard` 驱逐表状态，旧请求必须在 epoch 检查点快速失败或被 drain。此点并入 `eng-32`。 |
| 建议 4：in-flight 上限 + 队列深度指标 | **独有** | 仅 `fkv_config` 有 `max_write_concurrency` 字段且未接线（`fkv_config/src/lib.rs:42,113,283`，见 `eng-56`），无读侧 in-flight 上限。`ShardMetrics::observe` 只记延迟/错误（`fkv_metrics/src/lib.rs:257`），无队列深度/in-flight gauge。crossfire 有界队列只约束**未开始**请求，spawn 后 in-flight 无界，必须显式 semaphore。 |
| 验收 3 条（32 并发 get、慢 table scan 隔离、attach/detach 线性化） | **独有**，直接可用 | 落为 `eng-32` 的集成测试用例，放 `crate/engine/fkv_shard/tests/` |

### 采纳后的分阶段计划（全部落在 eng-32 轨道）

- **阶段 0 前置（不改 loop，属 `eng-41` + `eng-33` 子集）**
  1. per-table 打开状态 `OnceCell<Opened>` + 异步 open 锁，把 `ensure_table_ready` 的 `&mut` 需求从读路径剥离。
  2. 读路径 pin：`MappingSnapshot`/`PageRef` RAII 跨 `await` 持有，杜绝 reopen/compact 并发替换 mapping 的悬垂。
  3. `index_cache` 加代际号（见风险 2）。
  4. `fixed_buffer` 改为每请求独立/引用计数，消除 `GetLarge` 的 `&mut` 互斥。
- **阶段 1 读并发（`eng-32` 核心）**：`&Store` 共享 + `compio::runtime::spawn` 只放读类请求 + in-flight `Semaphore` + 关闭时 drain 再 `join`。
- **阶段 2 写串行化（`eng-33`）**：per-table FIFO + `max_write_concurrency` 接线（配合 `eng-56` 的配置补齐）。
- **阶段 3 优先级与预算（`eng-34`）**：高/低优先 ready 集合 + 时长预算 yield。
- **阶段 4 指标与验收**：in-flight/队列深度 gauge + 本节 3 条验收转测试。

### 风险

1. **取消安全**：`compio` 的 `JoinHandle` drop 即 cancel（`compio-executor-0.1.4/src/join_handle.rs:19-21, 148-153`）。用 `JoinSet` 收 in-flight 时，中途 drop 会取消正在写的任务，可能留半截落盘。必须先有 abort 回滚语义，或对写任务 `detach` 且由写门收敛。
2. **index_cache 代际**：当前 `RefCell` 借用都非跨 `await`（`fkv_task_read/src/lib.rs:89-97, 120, 195-197`），不会 panic。但并发化后「写侧 invalidate」与「读侧回填」可交错，让 stale `IndexNode` 复辟。需要 `(epoch, node)` 或写时递增代际校验。
3. **无请求级取消**：长 scan/archive 占用 in-flight 名额，`shutdown` 需等其自然结束。需配合 `eng-34` 分段 yield 或引入 `CancellationToken`（`eng-41`）。
4. **内存上限**：并发化后 in-flight 写任务各自持有 applying page/overflow/leaf_triple，无 `max_write_concurrency` 时峰值内存无界。
5. **收益前置成本高**：阶段 0 会引入每请求 pin 与打开状态查找开销，串行路径可能小幅回退，需用 `eng-32` 基准（32 并发 get 的 IO 深度、p99）验证净收益。

### 最终去向

~~移入 `issue/todo/03-shard-request-concurrency.md`，文件头语义为「已并入 `eng-32`，仅保留验收与指标附录，不单独排期」。~~

**作废**：验收与指标附录已由 `eng-32c` 独立承接，本文件移入 `issue/reject/`，见下节。

## 审查结论（2026-11-04）

### 判定

**拒绝（duplicate），`git mv` 到 `issue/reject/`。不写产品代码，不另开 todo。**

### 保留哪条、为什么

| 内容 | 保留位置 | 理由 |
|---|---|---|
| 串行 work loop 现象、C++ WorkLoop / `ExecuteReadyTasks` 对照、每请求 spawn | `issue/todo/eng-32-workloop-serialization.md` + `eng-32b-per-table-open-state-and-read-spawn.md` | 行号级证据、前置依赖、读/写分路均更全；本文措辞不可直接落地（`handle` 需 `&mut ShardState`） |
| per-table 写队列 / `max_write_concurrency` 背压 | `eng-33`（已 done）+ `eng-33a` / `eng-33b` | 代码已落地 `WriteGate`（`fkv_shard/src/lib.rs`），本文仅一句建议 |
| 高/低优先 ready 集与时长预算 | `eng-34` | 本文未涉及 |
| 3 条验收（32 并发 get、慢表 scan 隔离、attach/detach 线性化） | `issue/todo/eng-32c-acceptance-tests-and-inflight-metrics.md` | 逐条搬运，无遗漏 |
| in-flight 上限、ready 队列深度、每请求 IO 深度指标 | `eng-32c` + `eng-33b` | 同一缺口已登记 |

本文无剩余独有内容。留第二份 todo 会与 `eng-32c` 重复，违反 `eng-32` 审查结论
「`03` 只保留验收/指标附录，不复制 todo」。

### 代码现状核对（2026-11-04）

- `crate/engine/fkv_shard/src/lib.rs` 已有连续只读批 `MAX_READ_BATCH` / `run_read_batch`（`eng-32a`，done），
  以及 per-table `WriteGate::push` / `drain_write_gate`（`eng-33`）。
- 仍缺真正 `compio::spawn` 的跨轮并发（`eng-32b`）与 in-flight / 队列深度 gauge（`eng-32c`、
  `eng-33b`）。读批只在一轮内 `join_all`，不改变跨轮串行。
- 故验收 1「单 shard 32 并发 get，IO 并发在飞」须等 `eng-32b`，不能按本文原验收口径验收。

### 去向

`issue/reject/03-shard-request-concurrency.md`，仅作历史索引。
