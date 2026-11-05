# eng-34b per-segment `maybe_yield` 与 CPU 长段插桩

## 位置

- 已落地：`crate/engine/fkv_shard/src/lib.rs`（`Pri` / `pri_of` / 双通道 /
  `max_processing_time_us` 时间片）。
- 缺口：单条请求一旦开始就跑到结束，轮预算无法在其中让出。C++ 对应
  `include/storage/shard.h:76-79`（`CurResumeElapsedUs`）+ `src/tasks/task.cpp`
  的 `MaybeYield` + `FLAGS_eloqstore_yield_budget_us`（默认 20us）。

## 现象

`Get` 的 `store.get().await`、`Gc` / `Compact` 的文件循环、`scan` 的页循环在
单线程里是「一个 future 跑到底」。前台请求可以在**请求边界**插队（eng-34 已落），
但单个长请求（大 scan、大 archive、manifest 遍历）仍能独占 shard 线程。

## 前置（硬阻塞）

eng-32 的 ready set / 可抢占 poll 结构（`issue/todo/eng-32-workloop-serialization.md`
阶段 0/1）。没有多个可 poll 的 future，「让出」没有对象：compio 0.19 无公开
`yield_now`，`sleep(0)` 只是换一个 IO 让出点，不能调度别的请求。

## 方案（依赖前置落地后）

1. `TaskCtx { resume_start: Instant }`：每次被 poll 前刷新，对齐
   `cur_resume_start_us_`；不是创建时间。
2. `async fn maybe_yield(ctx)`：`resume_start.elapsed() >= yield_budget_us (20)` 时
   返回 `Pending`（登记 waker）或把当前任务降优先重入队（等价 `YieldToLowPQ`）。
3. 插桩点（只插 CPU 长段与批量 IO，不插每个 await）：
   - `fkv_store::append_mapping_log` 的 `dirty_page_li` 遍历；
   - `fkv_store::drop_table` 文件删除循环（每 N 个文件）；
   - `fkv_task_scan` 外层页循环；
   - `fkv_gc` / `fkv_task_compact` / `fkv_task_archive` 的 `file_li` 循环。
4. 配置：`yield_budget_us`（默认 20，`0` = 关闭），与
   `max_processing_time_us`（轮预算）分列，语义写清：轮预算管公平，段预算管 p99。
   接入 `KvOption` + ini 扩展键 + `fkv_runtime`。
5. 计时口径：只对 CPU 段计时（挂钟会因 IO 等待误判）；IO 密集任务按「每 N 次 IO」
   检查。

## 验收

- 低优先 archive 常驻 + 前台 get 混合：archive 进行中 get 的 p99 ≤ 无 archive 时
  的 1.2 倍。
- `yield_budget_us = 0` 时行为与 `eng-34` 落地后逐请求一致（无额外让出）。
- 审查项：所有 `for ... in file_li / dirty_page_li` 必须带 `maybe_yield`。

## 风险

- 插桩遗漏即失效，协作式让出没有兜底。
- 20us 段预算过小会引入频繁重调度；用 CPU 时间或仅 CPU 段计时。
- 不提供取消能力（见 eng-41），长 scan 仍不能被 abort。

## 阻塞（2026-11-04）

硬前置未落地，本条无法在本轮实施：

1. **eng-32 的 ready set / 可抢占 poll 结构未落地**：`eng-32-workloop-serialization.md`
   仍在 `issue/todo/`（`eng-32b` / `eng-32c` 同样未动）。核对当前
   `crate/engine/fkv_shard/src/lib.rs`，work loop 仍是「`recv` 一条 → `handle().await`
   跑到底」，`carry` / `wake_li` 只是请求边界上的重排，没有多个可 poll 的 future。
   没有并发的 poll 对象，段内让出（`Pending` / 重入队）没有语义，写了也只是空转。
2. **插桩点不在本会话地盘**：方案 3 要改 `fkv_store::append_mapping_log`、
   `fkv_store::drop_table`、`fkv_task_scan` 页循环、`fkv_gc` / `fkv_task_compact` /
   `fkv_task_archive` 的 `file_li` 循环；其中 `fkv_store` / `fkv_task_scan` 明令禁止本
   会话改（另一会话在写），其余属别的 crate，跨界改动既冲突又无法端到端验收。
3. **配置接线不完整**：`yield_budget_us` 要经 `KvOption` + ini 扩展键 + `fkv_runtime`
   → `ShardConfig`，其中 `fkv_runtime` / `fkv_config` 键位与 eng-34 的
   `max_processing_time_us` 分列需要先定稿；本轮只在 `fkv_shard` 加字段会成为零消费
   死配置（与 eng-33 同类问题）。

结论：待 eng-32（至少阶段 0/1 的 ready set）落地、且允许改 `fkv_store` /
`fkv_task_*` 后重启本条；届时按方案 1~5 实施。本轮不动代码。

## 复核（2026-11-04，协调者）

阻塞理由**部分过期**，逐条核对：

1. 「`yield_budget_us` 要经 KvOption + ini 扩展键 + `fkv_runtime` → `ShardConfig`，
   键位需要先定稿」——**已有现成先例，不需要再定稿**：
   `KvOption.max_processing_time_us`（`fkv_config/src/lib.rs:190`，标注 `[Ext: parse_num]`
   的 fkv 扩展键）→ `fkv_runtime/src/lib.rs:891-892` 透传进
   `ShardConfig.max_processing_time_us` → `fkv_shard/src/lib.rs:433`
   在**请求边界**用 `round_budget_due(...)` 让出一条低优先请求。
   照抄这一条链路加 `yield_budget_us` 即可（3 处小改），不会成为零消费死配置。
2. 「待 eng-32 的 ready set」——**这条仍然成立**：现有预算只在「一个请求跑完」的边界
   生效，eng-34b 要的是**任务内部**（每 segment / 每页）主动让出；那需要任务能拿到
   yield 点（eng-32 的 ready set / 序列化改造，或显式约定一个协作式 `MaybeYield`），
   否则只能在 shard 主循环里空转等待，等于换汤不换药。

结论：本条**不必再等键位定稿**；剩下只有「任务内 yield 点」一个真阻塞。
建议下一轮先落 `yield_budget_us` 的配置链路（与 `max_processing_time_us` 并列，
互不覆盖），yield 点等 eng-32 或单独立项。

## DONE（2026-11-05）

### 状态

已落地可配置的 per-segment 协作让出（`yield_budget_us`，默认 20us，`0` = 关闭），
并在 scan 页链、overflow / floor prev 链、GC / manifest / archive 文件遍历、
manifest 脏页编码等长循环插入 `maybe_yield`。读请求本身是独立 compio task
（eng-32b），让出点可让其它读任务与前台请求在长 scan / 大 value 读期间插入。

低优先 `Gc` / `Compact` 仍在 shard 主循环内串行执行：段内让出只把 CPU 长段切碎、
让 compio 调度器运行 IO 完成等其它任务，**不能**让另一条 shard 请求插进来。
真正的后台请求公平插队仍需 eng-32 的 ready set / 可抢占 poll 结构；本轮不做
假并发，限制见下。

### 改动

- `crate/engine/fkv_task_read/src/lib.rs`
  - 新增 `YieldBudget` / `yield_now` / `YIELD_CHECK_STRIDE`：`Instant` 段预算
    到点后自唤醒返回 `Pending`，把任务重新挂到 compio ready 队列尾部；
    `0` 预算不读时钟、不产生让出。
  - `ReadCtx` 增加 `yield_budget_us`；`read_overflow_budget` 每批 overflow 页
    检查一次；`read_floor_entry` 每个 prev 页检查一次。旧入口保留并委托 0 预算。
- `crate/engine/fkv_task_scan/src/lib.rs`
  - 新增 `scan_page_with_budget` / `scan_entries_with_budget`；旧入口保留。
  - 外层叶子页循环每页、页内 entry 循环每 `YIELD_CHECK_STRIDE` 条检查一次。
- `crate/engine/fkv_task_gc/src/lib.rs`
  - 新增 `manifest_retained_file_page_set_budget`、`gc_files_with_retained_budget`、
    `gc_segment_files_with_retained_budget`；旧入口委托 0 预算。
  - 每份 manifest、每个候选 data / segment 文件、retained file id 集合
    每 `YIELD_CHECK_STRIDE` 条检查一次。
- `crate/engine/fkv_store/src/lib.rs`
  - `Store::with_yield_budget_us` / `yield_budget_us`；默认 0（直接使用 Store 的
    单测行为不变），由 shard 注入。
  - 接线：scan（页 / 全量）、点读 `ReadCtx`、GC 三个入口。
  - 插桩：`append_mapping_log` 两处 `dirty_page_li`、`drop_table` 文件删除、
    `delete_branch` / `prune_archives`、`truncate_from` 批间、`clean_expired`
    过期项复核、`put_large_impl` 批次。
- `crate/engine/fkv_shard/src/lib.rs`
  - `ShardConfig.yield_budget_us`（默认 `DEFAULT_YIELD_BUDGET_US = 20`），
    通过 `Store::with_yield_budget_us` 注入。
  - `max_processing_time_us` 文档改为：轮预算管前后台公平，段预算管单请求 p99。
- 配置链路：`fkv_config::KvOption.yield_budget_us`（`[Ext: parse_num]`，
  默认 `DEFAULT_YIELD_BUDGET_US = 20`）→ `fkv_runtime` → `ShardConfig`。
  这是本 issue 必需的接线；`fkv_config` / `fkv_runtime` 各只加了一个字段 / 常量。

### 测试

- `crate/engine/fkv_store/tests/yield.rs`（新增）：
  - `scan_page_loop_yields_to_other_task`：`MemStore` 全部 IO 立即 `Ready`，
    budget=1 时标记任务在 4000 条 scan 完成前被 poll；
  - `overflow_chain_yields_to_other_task`：256KB overflow 链同样证明让出；
  - `zero_budget_disables_yield_points`：budget=0 时标记任务只能在长任务完成后
    运行，且结果与 budget=1 一致。
- `crate/engine/fkv_task_gc/tests/gc.rs`：新增 1000 个文件的 GC 让出 / `0` 预算
  不让出测试。
- `crate/engine/fkv_shard/tests/yield_budget.rs`（新增）：2000 条 Put 后，
  budget=20 / 0 两种配置的 Scan + Get 结果一致，验证配置链路。
- `crate/foundation/fkv_config/tests/config.rs`：`yield_budget_ext_key`（默认 20，
  ini `[run]` / `[permanent]` 可覆盖，0 = 关闭）。
- 通过：
  - `cargo test -p fkv_store --test yield --test overflow --test scan_pushdown
    --test ttl --test branch_archive --test large_value --test maintenance_batch
    --test gc_store --test truncate_drop --test store --test compio_store -- --test-threads=1`
  - `cargo test -p fkv_task_read --test read -p fkv_task_scan --test scan`
  - `cargo test -p fkv_task_gc --test gc -- --test-threads=1`
  - `cargo test -p fkv_task_write --test write -- --test-threads=1`
  - `cargo test -p fkv_shard -- --test-threads=1`（全部二进制通过）
  - `cargo test -p fkv_config --test config`
  - `cargo test -p fkv_runtime --no-run`
  - `cargo clippy -p fkv_store --lib --test yield -- -D warnings`；
    `cargo clippy -p fkv_task_read -p fkv_task_scan -p fkv_task_gc -p fkv_shard
    -p fkv_config -p fkv_runtime --all-targets -- -D warnings`
  - `rustfmt +nightly --edition 2024 --config-path rustfmt.toml` 仅对改动文件。

### 剩余 / 限制

1. **低优先请求仍不能段内插队**：`Gc` / `Compact` 执行时持有 `&mut Store`，
   shard 主循环正在 await 该 future；段内 `Pending` 只让 compio 调度器运行其它
   task，接收不到新的 shard 请求。要让后台请求在段内被前台挤掉，需要 eng-32
   的 ready set（多个可 poll 的请求 future）或把 GC 改成共享 `&Store` 的
   spawn task + 写屏障；本轮按用户要求记录最小实现、不改调度形态。
   最小实现：给 `Request::Gc` 一条「共享 Rc<Store> + finish_reads 屏障」路径，
   仅在无同分区排队写时提交，作为长后台任务在 `read_window` 中与前台读并发；
   完整公平调度仍需 ready set / 可抢占 poll。
2. **compact 内部循环未插桩**：`fkv_task_compact` 的 `file_li` / segment 循环不在
   本会话允许改动的 crate 列表内；store 侧一次 compact 是单 mapper 批次 + 单次
   manifest 提交，不能在外层切块而不破坏事务语义。后续可在 `fkv_task_compact`
   的 `file_li` 循环内复用 `YieldBudget`。
3. **单个超大 manifest replay 不可中断**：`fkv_task_reopen` 的 replay 是同步 CPU
   段，只能在 manifest 之间让出。
4. **p99 验收未做基准**：本环境没有 archive + get 混合的 p99 夹具；行为正确性、
   让出与 `0` 关闭已有测试，p99 ≤ 1.2x 的基准留待性能环境。
5. `fkv_store --all-targets` 的 clippy 被既有无关文件
   `tests/concurrent_snapshot.rs` 的 `len_zero` 警告挡住；未改该文件。

### 风险

- 协作式让出依赖插桩，漏点即失效；已按 `dirty_page_li` / `file_li` / 页链审查
  本轮可改范围。
- 计时用挂钟：IO 等待可能让一次检查偏早，最多多让出一次，不改变结果。
- `yield_budget_us` 过小会提高重调度频率；默认 20us 对齐 C++，
  可按 p99 调大或设 0 关闭。

## 复审（2026-11-05 r6b）

通过。四个 crate 的实现点与三组测试全部核实：

- `fkv_task_read/src/lib.rs:35` `YIELD_CHECK_STRIDE=256`、`:45 YieldBudget`、`:74 yield_now().await`（自唤醒回 ready 队列尾）、`:91`（compio 无公开 yield_now 的说明注释）；`ReadCtx` 承载预算，`read_overflow_budget`（`:226`）逐批检查。
- `fkv_task_scan/src/lib.rs:125 scan_entries_with_budget`、`:148 scan_page_with_budget`（旧入口委托 0 预算 `:121/:144`）。
- `fkv_task_gc/src/lib.rs:363 manifest_retained_file_page_set_budget`、`:462 gc_files_with_retained_budget`、`:509 gc_segment_files_with_retained_budget`（旧入口委托 `:355/:450/:497`）。
- `fkv_store/src/lib.rs:301 with_yield_budget_us`、`YieldBudget` 插桩（`:1516-1522` append_mapping_log 脏页遍历等）；`fkv_shard/src/lib.rs:72 DEFAULT_YIELD_BUDGET_US=20`、`:166` ShardConfig 字段、`:554` 注入；`fkv_config/src/lib.rs:259`（`[Ext: parse_num]` 键）。
- 测试：`fkv_store/tests/yield.rs:17-50 scan_yields`（标记任务在 4000 条 scan 完成前被 poll 的判定逻辑）、`:87/:92/:97` 三用例；`fkv_shard/tests/yield_budget.rs`、`fkv_config/tests/config.rs:485 yield_budget_ext_key` 在盘。
- 剩余限制（Gc/Compact 段内不可被前台插队、compact 循环未插桩、p99 基准未做）均如实写明为限制，未声称完成。
