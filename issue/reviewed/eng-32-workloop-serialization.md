# eng-32 WorkLoop 全串行：Rust 一次一个请求 vs C++ 多协程 IO 并发

## 位置
- Rust: `crate/engine/fkv_shard/src/lib.rs:154-185`（`while let Ok(envelope) = rx.recv().await` 内 `handle(&mut state, envelope.req).await`）
- C++: `src/storage/shard.cpp:115-212`（`WorkLoop`）、`1280-1331`（`ExecuteReadyTasks`）、`shard.h:110-111`（`ready_tasks_`/`low_priority_ready_tasks_`）

## 现状
Rust work loop 每次 `recv` 一个 envelope，然后 `handle(...).await` **完整跑完这个请求**才处理下一个。shard 内任何时刻只有一个请求在执行，读/写/GC/archive 全部严格串行，IO 无法重叠。

## C++ 实现
C++ WorkLoop 每轮：`Submit()` → `PollComplete()` → `ExecuteReadyTasks()`。`ExecuteReadyTasks` 从 `ready_tasks_`（高优先）与 `low_priority_ready_tasks_`（低优先）里 **反复 resume 多个协程**，直到超过 `FLAGS_max_processing_time_microseconds`。一个写协程在 `WaitIo` 让出时，读协程可被 resume 提交自己的 IO，多个请求的 IO 在单线程上重叠，`task_mgr_.NumActive()` 可 > 1。

## 差异与风险
- IO 并发丢失：C++ 单 shard 可同时有多个 in-flight IO（写页 + 读页 + manifest sync 重叠）；Rust 一个请求的 `.await` 链跑完前，其他请求拿不到线程。冷点读 p99、批量写吞吐直接受影响（todo §7.6 性能门槛）。
- 读阻塞写、写阻塞读：C++ 里读不排队等写完成；Rust 里一个大块写会让所有读请求在 `rx.recv()` 后排队。
- 后台任务（compact/gc/archive）与前台争用时，C++ 可被前台挤掉（低优先队列 + yield 预算），Rust 只能等前台跑完（见 eng-34）。
- 语义上结果正确（无数据竞争），但吞吐/尾延迟不达标。

## 对齐建议
- 在 shard loop 内用 `compio::spawn` 把每个请求包成独立 future 放进一个 ready 集合，loop 每轮 `poll` 所有 in-flight future（`futures::FutureExt::poll` 或手动 `Pin` 轮转），任一 `Poll::Ready` 即完成并回收，`Poll::Pending` 继续轮下一个。
- 引入高/低优先两个 ready 队列，前台请求高优先，后台（Gc/Compact/Archive/Reopen）低优先，对齐 `ready_tasks_`/`low_priority_ready_tasks_`。
- 加每轮处理时长预算（对齐 `FLAGS_max_processing_time_microseconds`），到点让出给 OS 线程，避免长时间占核。

## 性能优化建议
- 读路径天然可并发：点读/scan 只持 `&PageMapper`（无 pin，见 eng-41），spawn 后互不干扰，是最该并发的一类。
- 写路径仍按 eng-33 的每分区 FIFO 串行，但不同分区的写、以及写与读之间可并发。
- 用 `compio` 的 completion-based IO 天然适合“提交多个 op 等 completion”，比 C++ 的 poll loop 更易表达并发。

## 严重度
P1（行为不一致 / 性能不达标）

## 审查结论（2026-11-04）

### 总结论

**采纳，作为 shard 请求并发化的主轨道。** 现象、C++ 对照、性能影响均成立，C++ 证据已在 oracle 逐行核验。去向 `issue/todo/`。

与 `03-shard-request-concurrency.md` 的关系：两者重复。`03` 的审查已判定「主体并入 `eng-32`，仅保留验收、in-flight 指标、attach/detach 线性化作附录」，因此保留本条。本条信息更全的部分是 C++ 行级证据、前置依赖（27/core-11、eng-33、eng-34、io-07）和读/写分路策略。`03` 的三条验收与 in-flight 上限已并入下方计划，不另开轨道。

但原「对齐建议」有两处必须修正，否则无法编译：

1. **不能把 `handle(&mut ShardState, ...)` 直接 spawn。** `compio::runtime::spawn` 要求 `Future + 'static`，而 `handle` 收 `&mut ShardState`，读路径也会先走 `ensure_table_ready` 修改 `opened_table_set` 并可能 `reopen`/`open_branch`。必须先拆 per-table 状态。
2. **「读路径天然可并发」不成立。** `Store::get` 虽是 `&self`，但 `index_cache_map` 是 `HashMap<TableIdent, RefCell<IndexCache>>`，`RefCell` 非 `Sync`，且 `ShardState.fixed_buffer` 是单个 `Option<FixedBuffer>`，`GetLarge`/`PutLarge` 用 `as_mut()` 排他借用。并发读需要 per-table 缓存互斥（或线程独占）与按请求借还的 fixed buffer。

写队列、优先级/预算、快照/pin、fixed buffer 池化分别由 `eng-33`、`eng-34`、`27`/`core-11`、`todo/io-07` 承接，本条只负责「请求级并发骨架 + 读并发先落地」。

### 与 03 的去重判定

| 维度 | `03` | 本条 | 保留 |
|---|---|---|---|
| 串行 work loop 现象/影响 | 有 | 有 | 重复，本条 |
| C++ 执行模型证据 | 概念级 | 行号级，含高/低队列与预算 | 本条 |
| 每请求 spawn 建议 | 有，措辞同样过粗 | 有，需修正 | 本条修正后保留 |
| 每 table 写队列 | 有 | 仅引用 eng-33 | `eng-33` |
| 高/低优先与时长预算 | 无 | 有 | `eng-34` |
| 读 pin / 快照前置 | 无 | 有 | `27` + `core-11` |
| 验收用例（32 并发 get、scan 隔离、attach/detach 线性化） | 有，可直接执行 | 无 | 并入本条计划 |
| in-flight 上限与队列深度指标 | 有 | 无 | 并入本条计划 |

结论：`eng-32` 为主，`03` 作为验收与指标附录留在 `issue/todo/`，不单独排期。

### 逐条核验表

oracle 路径 `/Users/z/git/db/kv/eloqstore`，HEAD `82b774c`，与 `COMPAT_BASELINE` 一致。

| # | 原文主张 | 判定 | 证据 |
|---|---|---|---|
| 1 | Rust 每次 `recv` 一个 envelope，`handle(...).await` 跑完才处理下一个 | 成立，行号已漂移 | `crate/engine/fkv_shard/src/lib.rs:177`（`while let Ok`）、`:201`（`handle(&mut state, req).await`）、`:232-235`（回执后遇 Shutdown 才 break）。原文 `154-185` 已过期 |
| 2 | `handle` 完整执行 put/get/scan，含 page IO、manifest fsync、TTL | 成立 | `fkv_shard/src/lib.rs:349-613`，单分支一次跑到底 |
| 3 | C++ WorkLoop 每轮 `Submit` → `PollComplete` → `ExecuteReadyTasks` | 成立 | oracle `src/storage/shard.cpp:115-211`，三轮在 `:189-193` |
| 4 | `ExecuteReadyTasks` 反复 resume 高优先，超预算后跑低优先 | 成立，细节需补 | `shard.cpp:1280-1327`。`ready_tasks_` 循环到 `FLAGS_max_processing_time_microseconds` 后 break（`:1304`），再跑 `low_priority_ready_tasks_` 同预算（`:1322`）。release 预算 400us，debug 为 `UINT64_MAX`（`:45-52`） |
| 5 | 两个 ready 队列字段 | 成立 | `include/storage/shard.h:110-111` |
| 6 | 写协程 `WaitIo` 让出时，读协程可被 resume，单线程多 IO 重叠 | 成立 | `src/tasks/task.cpp:26-78`：`WaitIo` 置 `BlockedIO` 后 `Yield()` 回 `main_`，`FinishIo` 在 `inflight_io_==0` 时 `Resume()` 入 `ready_tasks_` |
| 7 | `task_mgr_.NumActive()` 可 > 1 | 成立 | `src/tasks/task_manager.cpp:153-157`（`GetReadTask` 自增 `num_active_`）；`shard.cpp:128` 的 idle 判据要求 `NumActive()==0` |
| 8 | Rust 单请求执行期间 IO 无法与其他请求重叠 | 成立 | 单 loop 串行，见第 1 条 |
| 9 | 「io_uring 并发深度 1」 | 基本成立，需限定 | 点读/写全程串行。`scan` 已有 1 页预取，单请求最多 2 个 in-flight（`fkv_task_scan/src/lib.rs:179-198`），跨请求仍为 1 |
| 10 | 读阻塞写、写阻塞读 | 成立 | 串行 loop 无读写分路 |
| 11 | 后台任务无法被前台挤掉，无 yield 预算 | 成立 | 全仓无 `YieldToLowPQ`/`MaybeYield`/`max_processing_time` 命中 |
| 12 | 建议每请求 `compio::spawn` 进 ready 集合，轮转 poll | 方向成立，原措辞不可直接落地 | `compio::runtime::spawn<F: Future + 'static>`（`compio-runtime-0.12.6/src/lib.rs:559`，无 `Send` 约束）。`handle` 需 `&mut ShardState`（`:349`），`Store::put` 需 `&mut self`（`fkv_store/src/lib.rs:817`），`ensure_table_ready` 也改状态（`:279-298`）。手工 `Vec<Pin<Box<dyn Future + '_>>>` 可避开 `'static`，但仍无法同时持有多个 `&mut ShardState` |
| 13 | 读路径只持 `&PageMapper`，spawn 后互不干扰 | 不成立 | `Store::get` 是 `&self`（`:406`），但 `read_ctx` 同时暴露 `&RefCell<IndexCache>`（`:130, :319`），`RefCell` 非 `Sync`；`fixed_buffer` 是单个排他 `Option`（`fkv_shard/src/lib.rs:400, :415`）；读请求入 `handle` 前仍要 `ensure_table_ready` |
| 14 | 写按分区 FIFO，不同分区写、读写可并发 | 方向成立，依赖未实现 | `eng-33` 仍在计划中，代码无 `WriteGate`/`Semaphore`/`PendingWriteQueue` |
| 15 | compio 的 completion IO 更适合表达并发 | 成立，但需上限与取消策略 | completion 模型可直接挂多个 op。`JoinHandle` drop 即 cancel（`compio-executor-0.1.4/src/join_handle.rs`），写任务不能随意取消 |
| 16 | 引入高/低优先 ready 队列与每轮时长预算 | 成立，但本条只应引用 `eng-34` | C++ 另有一条 20us 的 per-segment yield 预算（`shard.cpp:60-66`，`task.cpp:26-46`），`eng-34` 原描述未覆盖，落地时需补 |
| 17 | 严重度 P1 | 成立 | 无语义错误，是吞吐/尾延迟不达标，对齐 `03` 的复核结论 |

### 采纳后的分阶段开发计划

阶段 0 是硬前置。不拆状态，阶段 1 无法通过借用检查。

**阶段 0：并发化前置（阻塞项）**

1. COW 快照与 retire 链：推进 `todo/27` 与 `core-11`，读任务跨 `await` 持 `SnapshotRef`/`PageRef`，旧物理页最后引用释放才回池。
2. per-table 状态拆分：把 `Store` 的 `mapper_map`/`root_map`/`index_cache_map`/`compression_map`/`ttl_*` 聚成 `TableSlot`，按 table 独立持有。`ensure_table_ready` 改为幂等 single-flight（`OnceCell` + 异步打开锁），不再需要 `&mut` 整个 store。
3. `index_cache` 并发化：每 table `parking_lot::Mutex<IndexCache>` 或 shard 独占副本，写侧 `invalidate` 带 epoch/generation，防 stale 节点复辟。
4. fixed buffer：改为每请求从池借出、RAII 归还，对齐 `todo/io-07`，消除 `GetLarge`/`PutLarge` 的 `&mut` 互斥。
5. attach/detach 屏障：引入 shard epoch。`AttachShard`/`DetachShard` 提升 epoch，旧请求在检查点快速失败或被 drain；`Shutdown` 先停止收新请求，再等在飞请求结束。

验收门槛：并发 reopen/compact 与读同时运行，读结果自洽，无借用冲突，无 stale index 命中。

**阶段 1：读并发（本条核心）**

- 只把 `Get`/`GetLarge`/`Scan`/`Floor` 放进 ready 集合，用 `compio::spawn` 或手工 `Pin` 轮转。写类仍走串行 `handle`。
- 增加读侧 in-flight `Semaphore`，上限可配置，避免 spawn 后无界占用内存与 fd。
- 关闭流程改为 drain：停止 `recv`，等待 in-flight 读完，再处理 `Shutdown` 并 join。
- 指标增加 in-flight gauge、ready 队列深度、每请求 IO 深度。

验收门槛：单 shard 32 并发 get 时 `MeteredIo` 观测到 IO 深度 > 1；慢 table 的 scan 不阻塞其他 table 的 get；attach/detach 与并发请求线性化测试通过。

**阶段 2：写串行化（接 `eng-33`）**

- 每 table FIFO + `running_` 门，同一 table 写严格串行，不同 table 写可交错。
- 接入 `max_write_concurrency` 作为内存背压，超限请求停留队列。
- OOM 回滚 + 重试语义保持，重试请求排在队尾，让其他任务释放 pin。

验收门槛：同 table 写顺序与单线程一致，不同 table 写 IO 重叠，峰值内存受 `max_write_concurrency` 限制。

**阶段 3：优先级与协作让出（接 `eng-34`）**

- 高/低优先 ready 两个队列，前台读高优先，compact/gc/archive/clean-expired 低优先。
- 每轮预算对齐 `max_processing_time_microseconds`，release 默认 400us，可配置。
- 长任务每 20us 左右主动让出，对齐 `eloqstore_yield_budget_us` 与 `MaybeYield`。

验收门槛：compact/GC 期间前台 get 的 p99 不劣化，低优先任务不饿死。

**阶段 4：指标、基准与回归**

- 把 `03` 的三条验收转成 `crate/engine/fkv_shard/tests/` 集成测试。
- 基准对比串行与并发：点读 p50/p99、批量写吞吐、scan 隔离、单 shard IO 深度。
- 增加队列深度、in-flight、取消/排空计数指标。

### 风险

1. **取消安全**：`compio` 的 `JoinHandle` drop 会取消任务。读可取消，写任务不能中途 drop，否则可能留下半截落盘。写任务要么 `detach` 并由写门收敛，要么先补 abort 回滚。
2. **重构面大**：per-table 状态拆分触及 `Store` 全部 42 个 `&mut self` 接口，回归风险高。建议先以 `TableSlot` 包一层，保持串行路径行为不变，再逐类打开并发。
3. **借用模型限制**：多请求共享 `Rc<RefCell<TableSlot>>` 会把写路径变成运行期 `borrow_mut` 冲突。优先 per-table 独占（表任务）或读快照，避免把编译期错误推成 panic。
4. **快照与 index cache 一致性**：读必须持有与 root 同代的 mapper 与 index cache，写发布 root 前要完成 cache invalidate，否则读到旧 mapping 对应的新页。
5. **内存与 fd 上限**：并发化后 in-flight 写各自持有 applying page/overflow/leaf_triple，读各自持有页缓冲。没有 `Semaphore` 与 `max_write_concurrency` 时峰值无界。
6. **收益不确定**：本地 NVMe 上单请求串行链可能已接近带宽上限，并发化主要改善尾延迟与读写互阻。阶段 0 会引入 pin 与 per-table 查找开销，必须用基准验证净收益，避免为并发而并发。
7. **配置与语义漂移**：`max_processing_time_microseconds`、`eloqstore_yield_budget_us` 在 C++ 是 gflags，Rust 需接线到 `fkv_config`，并补齐 `eng-56` 指出的配置未接线问题。

### 最终去向

移入 `issue/todo/eng-32-workloop-serialization.md`。`03-shard-request-concurrency.md` 保留为验收与指标附录，不单独排期。

### 落地拆分（2026-11-04）

上面「最终去向」的单文件计划已按可执行粒度拆分，避免一个 todo 同时含已可落地与阻塞项：

- `eng-32a-read-batch-join-all.md`：**本轮实现**。连续只读请求共享 `&Store`，`join_all`
  同轮推进，不 `spawn`、不拆状态、不改写路径语义。已完成，见该文件 `## DONE`。
- `eng-32b-per-table-open-state-and-read-spawn.md`：真正 `spawn` 的阻塞前置
  （`eng-73` / `27` / `core-11` / `io-07` / attach epoch），未排期。
- `eng-32c-acceptance-tests-and-inflight-metrics.md`：`03` 的验收与 in-flight /
  队列深度指标附录，不单独排期。

与 `03-shard-request-concurrency.md` 的重叠关系：`03` 仅保留验收与指标附录，主体全部由本
issue（及上面的 32a/32b/32c）承接，不复制第二份 todo。

## DONE（2026-11-05 文档对账）

工作循环的读并发骨架已落地，旧「一次只处理一个请求」结论不再成立：

- **读请求 spawn**：`ReadFlight` 用 `FuturesUnordered` 持有独立 compio task，
  窗口上限 `MAX_READ_BATCH = 32`；读任务只持 `Rc<Store>`，写 / 控制请求开始前
  先 `finish_read_li` 等在飞读，保证 `Rc::get_mut` 与 attach/detach 线性化。
  `crate/engine/fkv_shard/tests/read_spawn.rs` 覆盖 96 并发读、慢 scan 不阻塞
  其他表 get、detach 与在飞读线性化。
- **高/低优先与预算**：`tx_hi` / `tx_lo` 双通道，`max_processing_time_us`（默认
  400us）到点先插低优先；单请求内 `YieldBudget`（默认 20us）在 scan / overflow /
  GC 长循环主动让出。`tests/priority.rs`、`tests/yield_budget.rs` 覆盖。
- **写路径**已进 per-partition `WriteGate` 并做同分区 group commit；跨分区写的
  真正并行不在本条范围内，由 [`eng-33a`](./eng-33a-write-gate-concurrent-dispatch.md)
  承接。

本轮复跑 `cargo nextest run -p fkv_shard --test read_spawn --test priority
--test write_gate --test read_batch --test yield_budget`：12/12 passed（2026-11-05）。

仍缺的读侧 in-flight / 读批大小指标留在
[`eng-32c`](./eng-32c-acceptance-tests-and-inflight-metrics.md)。

## 复审（2026-11-05 r7）

结论：**通过（读并发骨架 + 高/低优先 + yield 预算，按拆分范围验收）**。核当前
源码（均亲读）：

- 读请求 spawn：`fkv_shard/src/lib.rs:2008-2011` `ReadFlight` 持
  `FuturesUnordered<ReadTask>`；窗口上限 `MAX_READ_BATCH = 32`（`:59`，
  `is_full` `:2025-2027`）；每条读 spawn 独立 compio task、只持 `Rc<ShardStore>`
  （`ReadTask::new` `:1970-1973`）；写/控制请求先 `finish_read_li` 保证
  `Rc::get_mut` 与线性化（主循环 `:604-636`）。
- 高/低优先与预算：`tx_hi`/`tx_lo` 双通道（`:290-292`、`:469-470`）；
  `round_budget_due(max_processing_time_us)` 到点先插一条低优先并计数/重置
  （`:664-676`）；默认 400us = `fkv_types/src/lib.rs:37`
  `DEFAULT_MAX_PROCESSING_TIME_US`；`YieldBudget` 默认 20us =
  `fkv_config/src/lib.rs:57-58` `DEFAULT_YIELD_BUDGET_US`。
- 写路径 per-partition `WriteGate` + 同分区 group commit（`:596` 构造，
  `drain_write_gate` `:644-655`）；跨分区写真正并发按拆分归 `eng-33a`（todo，
  口径一致）。
- 测试：`tests/read_spawn.rs` 3 条（96 并发首读只 replay 一次 `:127`、慢 scan
  不阻塞他表点读 `:159`、detach 与在飞读线性化 `:237-287`）；`tests/priority.rs`
  3 条（Get 插队断言「后台完成 < LOW_NUM/2」`:140-144`）；`tests/yield_budget.rs`
  1 条；`tests/write_gate.rs` 3 条；`tests/read_batch.rs` 2 条——合计 12，与 DONE
  「12/12」一致。残余 in-flight/读批指标在 `eng-32c`（todo），不阻塞本条。
