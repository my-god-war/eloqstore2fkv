# eng-34 无低优先队列与协作式 yield 预算

## 位置
- Rust: `crate/engine/fkv_shard/src/lib.rs:154-185`（work loop 无优先级、无时长预算、无 yield）；全仓无 `YieldToLowPQ`/`MaybeYield`/`max_processing_time`。
- C++: `src/storage/shard.cpp:1280-1331`（`ExecuteReadyTasks` 高低优先两队列 + `FLAGS_max_processing_time_microseconds`）、`shard.h:64-79`（`CurResumeElapsedUs`）、`src/tasks/batch_write_task.cpp:254`（`YieldToLowPQ`）、`write_task.cpp`（`MaybeYield`/`Yield`）。

## 现状
Rust work loop 无优先级区分，所有请求一视同仁按入队顺序处理；没有“一个任务占用线程超过 N 微秒就让出”的协作式预算。

## C++ 实现
C++ `ExecuteReadyTasks` 先跑 `ready_tasks_`（前台），每轮检查 `DurationMicroseconds(ts_) >= FLAGS_max_processing_time_microseconds`，到点 break 并至少跑一个 `low_priority_ready_tasks_`（后台）防饿死。`WriteTask::Apply` 开头 `YieldToLowPQ()` 主动降优先。`CurResumeElapsedUs` 让长任务被 `MaybeYield` 打断。

## 差异与风险
- 后台任务（compact/gc/archive/reopen）与前台写/读争用线程时，C++ 能被前台挤掉或限时；Rust 里后台任务一旦开始就跑到结束，前台请求只能等。
- 长尾：一个大 scan 或大 archive 会阻塞同 shard 所有请求，p99 不可控（todo §7.6、风险 R10）。
- 与 eng-32 叠加：即使未来并发化，若无 yield 预算，单个长 future 仍可独占 poll 循环。

## 对齐建议
- 引入高/低优先 ready 集合（见 eng-32），work loop 每轮先 poll 高优先，达到预算（如 1ms）后 poll 一个低优先。
- 给写/后台任务一个“每 N 次 IO 或每 X 微秒主动 yield”的钩子（等价 `YieldToLowPQ`/`MaybeYield`），用 `Instant` 记录 resume 起点（对齐 `cur_resume_start_us_`）。
- 预算值做成可配置（对齐 `FLAGS_max_processing_time_microseconds`）。

## 性能优化建议
- 前台读设高优先、后台 compact/gc 设低优先，保证 p99；archive 这种大任务强制低优先 + 分段 yield。
- yield 用 `compio` 的 timer 或 `std::thread::yield_now`（shard 线程）实现，避免阻塞 runtime。

## 严重度
P2（性能 / 健壮性）

## 审查结论（2026-11-04）

### 总结论

**部分采纳**。事实核验基本成立（Rust 无优先级 / 无预算 / 无 yield；C++ 双队列与非对称预算属实），但本条 issue 有 4 个必须修正的点，且**强依赖 eng-32 先落地**，当前不能单独开发。

修正点：

1. **重复 eng-32**：eng-32 的「对齐建议」已含「引高/低优先两个 ready 队列」+「每轮处理时长预算」。eng-34 若照原文实施会与 eng-32 撞车。eng-34 的真实独立增量只有 C++ 的第二套预算：`eloqstore_yield_budget_us`（每段 20us，驱动 `MaybeYield`/`YieldToLowPQ`），原文完全未提。
2. **预算值与语义写错**：原文只提 `FLAGS_max_processing_time_microseconds`（建议 1ms）。实测 release 默认 **400us**，debug 默认 `UINT64_MAX`（等于关闭）；另有独立的 **20us** per-segment 预算。前者是调度器轮间预算，后者才管 p99 尾延迟；两者不可合并成一个数。
3. **`YieldToLowPQ` 不是「yield 到 OS」**：它是 `low_priority_ready_tasks_.Enqueue(this)` + 让回主循环，等于把当前任务降优先重排队。原文「用 compio timer 或 `std::thread::yield_now` 实现」不准确：compio 0.19.2 / compio-runtime 0.12.6 **没有公开 `yield_now`**；`sleep(Duration::ZERO)` 会走 timer 驱动，非零成本；单 shard 只有 1 个 future 时两种 yield 都无对象可让。
4. **原文高估了「后台任务跑到结束」**：Rust 现有长任务已经是 async，每个 IO await 天然是让出点（`drop_table` 逐文件 await、scan 逐页 await 含 prefetch）。真正无让出点的是 **CPU 段**（manifest builder 遍历 dirty pages、压缩批、TTL 过滤循环）。所以 yield 钩子的插桩目标应改为「CPU 长段 + 每 N 次 IO」，而不是「给协程加 yield」。

### 逐条核验表

| # | 原文断言 | 核验 | 证据 |
|---|---|---|---|
| 1 | Rust work loop 无优先级、无预算、无 yield | ✅ 成立 | `crate/engine/fkv_shard/src/lib.rs:177` `while let Ok(envelope) = rx.recv().await`，一次 `handle(...).await` 跑完；全仓 rg `YieldToLowPQ\|MaybeYield\|max_processing_time\|yield_now\|low_priority` 在 `--type rust` 下 0 命中 |
| 2 | C++ `ExecuteReadyTasks` 先跑 `ready_tasks_`，超预算 break | ✅ 成立 | `eloqstore/src/storage/shard.cpp:1283-1309`，`DurationMicroseconds(ts_) >= FLAGS_max_processing_time_microseconds` 后 break |
| 3 | 到点后至少跑一个低优先防饿死 | ✅ 成立（但机制不同） | `shard.cpp:1311-1326` 不是「只跑一个」，而是**整个低优先队列继续跑到同预算**；注释写「at least one」是保守说法 |
| 4 | 预算值 ≈ 1ms | ❌ 错 | `shard.cpp:45-52`：release **400us**，debug `UINT64_MAX`。1ms 无来源 |
| 5 | `CurResumeElapsedUs` 让长任务被 `MaybeYield` 打断 | ✅ 成立，但漏了第二预算 | `include/storage/shard.h:76-79`；`src/tasks/task.cpp:618-631` `MaybeYield` 用的是 `FLAGS_eloqstore_yield_budget_us`（`shard.cpp:56-63` 默认 **20us**），**不是** `max_processing_time_microseconds` |
| 6 | `WriteTask::Apply` 开头 `YieldToLowPQ()` | ⚠️ 归属错 | 开头主动降优先的是 `BatchWriteTask::Apply`（`src/tasks/batch_write_task.cpp:252-255`）；`WriteTask` 是在 `WritePage` 后按 `inflight_io_ >= max_write_batch_pages` 决定 `WaitWrite` 还是 `YieldToLowPQ`（`write_task.cpp:305-314`） |
| 7 | `YieldToLowPQ` = 降优先重排队 | ✅ 成立 | `src/tasks/task.cpp:40-44` `low_priority_ready_tasks_.Enqueue(this)` → `main_.resume()` |
| 8 | Rust 可改用 compio timer / `thread::yield_now` | ⚠️ 不准确 | `compio-0.19.2` 仅 re-export 子 crate，无 `yield_now`；`compio-runtime-0.12.6` 只有 `spawn`/`sleep`；`compio-executor` 的 `yield_now` 是**私有**内部自旋助手 |
| 9 | 与 eng-32 叠加后单长 future 仍独占 poll 循环 | ✅ 成立 | 与 6 同证：无抢占、无优先级 |
| 10 | 全仓无相关实现（原文「全仓无」） | ✅ 成立 | 同上 rg；`crate/engine/fkv_task/src/lib.rs` 仅一行文档注释（属 32-stub 问题），yield/优先级无落点 |

### 与 eng-32 / 03 / 33 的依赖关系

```
32 (fkv_task 抽象: Task trait / yield 落点)
   └─► eng-32 (多 future ready 集合 + 优先级队列 + 轮预算)   ← eng-34 的硬前置
          └─► eng-34 (per-segment yield 预算 + MaybeYield 插桩 + 预算可配置)
03  ≡ eng-32（同一条问题的另一份描述，应合并）
eng-33 (写队列 / WriteGate)  并行，不阻塞 eng-34，但决定「写请求初始进哪个队列」
eng-41 (读无 pin / 无取消)   相关但正交；`MaybeYield` 不能替代取消
```

- **硬前置 = eng-32**：ready 集合里只有 1 个 future 时，「预算到点 poll 低优先」与「当前任务主动让出」都没有对象可让。没有 eng-32，eng-34 任何改动都是空转。
- **eng-32 已吃掉一半**：双队列与轮预算写在 eng-32 的对齐建议第 2、3 条。落地时应由 eng-32 实现，eng-34 只保留增量。
- **与 03 去重**：03 与 eng-32 是同一问题，建议合并后再排 eng-34。

### 采纳后的开发计划（依赖前置，顺序不可交换）

**Phase 0（前置，属 eng-32/32）**

1. `fkv_task` 落 `TaskCtx { resume_start: Instant, pri: Pri }` 与 `async fn maybe_yield()`；不实现具体调度。
2. eng-32 落 `ReadySet { high: VecDeque<Task>, low: VecDeque<Task> }`，工作循环每轮：poll 高优先至 `round_budget_us`，再 poll 低优先至少 1 个。验收：单 shard 32 个 get 在飞；慢 scan 不阻塞其他 get。

**Phase 1（eng-34 增量：预算与插桩）**

3. `ShardConfig` 增加 `max_processing_time_us: u64`（默认 **400**，0 = 关闭，对齐 C++ release 值；不照搬 debug 的 `UINT64_MAX`）与 `yield_budget_us: u64`（默认 **20**）。二者语义写清：轮预算管公平，段预算管 p99。
4. `maybe_yield()` 实现：`resume_start.elapsed().as_micros() >= yield_budget_us` 时 `yield_impl()`；`Pri::High` 任务降为 `Pri::Low` 重入队（等价 `YieldToLowPQ`），`Pri::Low` 任务轮转到队尾。计时基准对齐 `cur_resume_start_us_`：**每次被 poll 前刷新**，不是创建时。
5. 插桩点（只插 CPU 长段与批量 IO，不插每个 await）：
   - `fkv_store/src/lib.rs:1024` `append_mapping_log` 的 `dirty_page_li` 遍历（改 async 或按阈值切块）；
   - `fkv_store/src/lib.rs:2127` `drop_table` 文件删除循环（每 N 个文件一次）；
   - `fkv_task_scan/src/lib.rs:177` 外层 `loop` 每页一次（`prefetch` 之外的 CPU 段）；
   - gc / compact / archive 的 file_li 循环（`fkv_gc`、`fkv_task_compact`、`fkv_task_archive`）。
6. 优先级初始分类：读/点查 = High；`Gc/Compact/CreateArchive/DeleteBranch/CleanExpired` = Low；写类初始 High（写完成后按 4 降 Low），**不要**静态把写判为 Low，否则与 C++ 语义不符（C++ 是写任务自降，不是入队即低）。
7. 指标：`shard_yield_total`、`shard_round_over_budget_total`、`shard_low_pq_depth`。

**Phase 2（验证）**

8. p99 夹具：低优先 archive 常驻 + 前台 get 混合，验收「archive 进行中 get p99 ≤ 无 archive 时的 1.2 倍」。
9. 预算关闭（0）时行为与现状逐字节一致，防回归。

### 风险

- **依赖风险（最高）**：eng-32 未落地前动工 = 无效改动；eng-32 若选择 `compio::spawn` + 运行时调度（而非自建 ready 集合），则**无法实现优先级**（compio 执行器队列 FIFO 无优先级），eng-34 的方案要跟着改成自建 poll 集合。**开发前必须先定 eng-32 的调度形态。**
- **预算过小**：20us 段预算 + `sleep` 走 timer 驱动，可能引入额外 syscall；Rust 侧应按 CPU 时间而非挂钟近似，或仅对 CPU 段计时，否则 IO 密集任务会频繁空转。
- **插桩遗漏即失效**：`maybe_yield` 是协作式的，漏一处长循环就等于该路径无预算；需以「所有 `for ... in file_li/dirty_page_li` 必带钩子」为审查项。
- **公平性副作用**：写任务降 Low 后可能与后台任务同队，若低优先队列无内部 FIFO 保证，会破坏 eng-33 的每分区写顺序。低优先队列必须仍按分区 FIFO。
- **配置面扩大**：两个新配置项需同时接入 `fkv_runtime`/attach 路径与默认值校验，避免出现 0 与默认混淆。
- **不解决的事**：本条不提供取消能力（见 eng-41），长 scan 仍不能被 abort；不要把它当取消机制用。

## 实施结论（2026-11-04，保守子集）

原审查「硬前置 eng-32」的结论对**双队列 + 时间片**这一半不成立：准入级优先级
不需要 ready set，已落地。per-segment 抢占仍需要可抢占结构，拆出 `eng-34b`。

### 落地内容

- **双通道**：`ShardSender` 按 `pri_of` 分 `tx_hi` / `tx_lo`（各自 `queue_size`
  背压）。work loop 先抽高优先；低优先只在没有前台请求，或高优先连续服务超过
  `max_processing_time_us`（默认 400us，`0` = 关闭）时跑一条。对齐 C++
  `ExecuteReadyTasks`「超预算后至少跑一个低优先防饿死」。
- **一次一条**：低优先写经 `WriteGate` 时一次只跑一条，剩余由循环顶
  `has_ready()` 继续驱动，每轮都重新检查高优先通道。
- **优先级分类**：`Gc` / `Compact` / `CreateArchive` / `DeleteBranch` /
  `CleanExpired` = Low；读/写/控制（含 `Reopen` / `AttachShard` /
  `DetachShard` / `Shutdown`）= High。写**不**静态降 Low（C++ 是写任务运行中
  `YieldToLowPQ`），保持 eng-33 每分区 FIFO 与写延迟语义。
- **空闲等待**：`futures::select` 同时看两条通道，一侧关闭后继续抽另一侧；
  poll 栈内不阻塞、不内联收割调度器（compio 纪律）。
- **配置**：`ShardConfig::max_processing_time_us`、`KvOption.max_processing_time_us`
  （ini 扩展键同名，`[run]` / `[permanent]` 均接受）、`fkv_runtime` 接线，
  缺省回退 `fkv_shard::DEFAULT_MAX_PROCESSING_TIME_US = 400`。

### 验收

- `crate/engine/fkv_shard/tests/priority.rs::get_overtakes_queued_background_tasks`：
  4096 条 `Gc` 积压时，`Get` 完成时后台执行数 < 积压的一半（无优先级时必然排到
  队尾）。
- `background_only_request_is_served`：只有低优先请求时 shard 仍能被唤醒执行。
- `cargo nextest run -p fkv_shard -p fkv_runtime -p fkv_store --no-fail-fast`：
  136 passed / 1 skipped。
- `cargo clippy -p fkv_shard --no-deps --all-features --all-targets -- -D warnings`
  零警告。

### 与原审查的差异

- 两个预算**仍不可合并**：本次只落轮预算（请求边界时间片）；per-segment 20us
  预算需要 `maybe_yield` 在 CPU 长段内真正让出，留 `eng-34b`。
- 「YieldToLowPQ 不是 OS yield」成立：本次实现为「低优先通道重排队」，没有用
  `thread::yield_now` / `sleep(0)`。
- 「真正无让出点的是 CPU 段」成立：插桩留 `eng-34b`。
- 原文「预算 ≈ 1ms」不采用，默认 400us（对齐 C++ release）。

## DONE（2026-11-04）

保守子集已实现并合入：高/低优先双通道 + 请求边界时间片 + 低优先一次一条。
per-segment `MaybeYield` 与 p99/指标见 `issue/todo/eng-34b`、`issue/todo/eng-34c`。

## 复审（2026-11-05 r6b）

通过。

- 双通道：`ShardSender` 按 `pri_of`（`crate/engine/fkv_shard/src/lib.rs:87`）分 `tx_hi/tx_lo`（`:283-285`、建组 `:462-463`）；work loop 高优先连续服务超 `max_processing_time_us` 先让一条低优先（`:590`、`round_budget_due :654`）。
- 配置：`ShardConfig.max_processing_time_us`（`:160`，默认 400 对齐 C++ release）、常量上移 `fkv_types/src/lib.rs:37`；`fkv_runtime/src/lib.rs:1152-1153` 透传，`KvOption` 扩展键在 `fkv_config`。
- 测试：`crate/engine/fkv_shard/tests/priority.rs:106` `get_overtakes_queued_background_tasks`（`:141` 断言后台执行数 < 积压一半）、`:155` `background_only_request_is_served`、`:180` 调度指标用例。
- 段预算（eng-34b）与指标（eng-34c）均已落地并入 `issue/reviewed/`。
