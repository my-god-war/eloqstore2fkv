# eng-33a 写队列真正并发派发（跨分区写并行 + 全局背压）

父 issue：`eng-33-write-gate-missing.md`（已落地串行形态，见 `issue/done/`）。

## 现状

`fkv_shard/src/write_gate.rs` 已有 per-partition FIFO 队列、`running` 门与全局计数上限，
`ShardConfig::max_write_concurrency` 已从 `fkv_config` 接线（0 = 不限）。但 work loop
仍是串行 `await`：`drain_write_gate` 一次只跑一条写，跨分区写没有真正并行，
`running` 恒为 0/1，上限只有计数意义。

## 阻塞依赖

1. **per-table 状态拆分 + 读路径 spawn**：`eng-32b`（`Store::put` 目前要 `&mut self`，
   无法同时持有多个写 future）。
2. **COW 快照 / pin**：`27` + `core-11`，写并发时读需看到稳定 root。
3. **fixed buffer 池化**：`io-27`，消除 `PutLarge` 对 `fixed_buffer.as_mut()` 的排他借用。

## 方案

- `drain_write_gate` 改为「取到就派发」：`take_next()` 后把 `run_gated_write` 包成
  独立 task（`compio::spawn` 或 eng-32 的 ready 集合），完成时调用 `gate.finish(tbl)`
  并续跑同分区队首。
- 全局额度改由真正的许可控制：`take_next` 返回 `None` 时不再自旋，等任一写完成
  后由 `finish` 重新驱动（当前实现已按此语义写好 `ready_li` 回退）。
- 写任务不可中途 drop：`JoinHandle` drop 即取消，可能留半截落盘。Shutdown 必须
  「停止收新写 → 等全部 in-flight 写完成 → 清 `pending` 并回 `NotRunning`」。
- 同一分区仍严格 FIFO；跨分区按 `ready_li` 顺序轮转，避免大分区饿死小分区。

## 与 eng-34 的耦合

- eng-34 的低优先队列必须保持**分区 FIFO**；写任务初始 High、完成前自降 Low
  （对齐 C++ `BatchWriteTask::Apply`），不要把写静态判为 Low。
- 优先级调度换掉驱动方时，`WriteGate` 的 `running`/额度语义不能丢。

## 验收

- `MeteredIo` 观测到同 shard 两个分区写 IO 重叠；`max_write_concurrency = 1` 时无重叠。
- 同分区 N 条写严格按入队顺序落盘（探针记录完成顺序）。
- `Drop` / `DetachShard` / `Shutdown` 清队列后，排队写回 `NotRunning`，无挂起 future。
- 峰值内存随 `max_write_concurrency` 单调受限（eng-32c 指标）。

## 阻塞（2026-11-04）

「取到就派发」需要同时持有多个写 future，当前代码结构做不到，且前置未落地：

1. **`Store::put` 仍需 `&mut self`**（核对 `crate/engine/fkv_store/src/lib.rs:1100`
   `pub async fn put(&mut self, tbl, batch)`）。`run_gated_write` 拿的是
   `&mut ShardState`（内含 `&mut Store`），无法把两条写包成并发 task；这正是本条
   阻塞依赖 1（`eng-32b`）要拆的 per-table 状态。`eng-32b` 仍在 `issue/todo/`
   且自述「阻塞项，暂不排期」，其依赖（`eng-73` per-table TableSlot、读路径
   single-flight、`eng-32c` in-flight 上限）也未落地。
2. **驱动方缺失**：方案要求用 `compio::spawn` 或 `eng-32` 的 ready 集合接住写
   task；`eng-32` 的 ready set / 可抢占 poll 结构同样未落地（见本轮
   `eng-34b` 的阻塞说明）。没有它，`take_next` 返回的那条写仍只能在主循环里
   `await` 到底，`running` 无法真正 > 1。
3. **写并发语义耦合**：写并发化要求 COW/pin（`27` + `core-11`）保证跨 await 的读
   看见稳定 root，以及 `PutLarge` 的 fixed buffer 排他借用消除（`io-27` 已落，
   但 `eng-32b` 仍把 `io-07` 列为依赖）。这些不齐时强行并发会引入数据竞争，不是
   单纯改 `drain_write_gate` 能收口的。
4. **旁路读取**：`fkv_store` 属于本会话禁改 crate（另一会话在写），实现必然要改
   它（`put` 借用形态 / TableSlot）。

结论：本条必须与 `eng-32b`（及其上游 `eng-73` / `27` / `core-11` / `io-07`）
捆绑实施，现在重启只会把「全局串行」换成「同分区 FIFO 的串行」，行为不变、风险
净增（与 `eng-33` 父 issue 审查结论一致）。本轮不动代码，留在 `todo/`。

## DONE（2026-11-05）

### 状态

跨分区写真正并发落地：高优先 `Put` 组取到就派发成独立 compio task，每个任务
独占目标分区的 `Store`；同分区仍由 `WriteGate::running` 严格 FIFO 串行。
`max_write_concurrency` 从「只计数」变成真正的派发额度（`= 1` 时跨分区也不
重叠）。读 / 控制 / 归档 / term 边界未改语义。

### 关键设计：每分区 Store（不拆 `Store` 内部）

之前无法并发持有多个写 future 的根因是 `Store::put` 需要 `&mut self`。本轮
按「每分区一份 `Store`」收敛，仍保持一个 shard 一份 IO 后端与一份 index 缓存
字节账本：

- `crate/engine/fkv_shard/src/partition.rs`（新增）
  - `PartitionStoreMap`：`TableIdent -> Slot { Option<Rc<Store>>, cache_stat, touch }`。
    写任务运行期间槽位为 `None`（`is_writing`），读任务持 `Rc` 克隆共享同一分区
    Store；写前 `take`（`Rc::try_unwrap`）交给任务，完成 `put_back`。
  - 每分区 Store 共享 `MeteredIo<StoreBackend>` 克隆（fd 缓存 / 路径索引 /
    io 层写闸门仍是全 shard 一份）与 `Arc<CacheLedger>`。
  - 新建分区 Store 时用 `Store::adopt_attach` 采纳当前 branch / term，不重复
    清理共享 IO 缓存（attach 时统一失效一次）。
  - root meta 字节预算由 shard 级近似 LRU 执行（单表 Store 无法驱逐自己）；
    只驱逐空闲（无读者、无写任务）分区。
- `crate/engine/fkv_shard/src/backend.rs`
  - `StoreBackend` 三个变体改 `Box` → `Rc` 并 `derive(Clone)`；克隆只增引用计数，
    让每分区 Store 共享同一后端。
- `crate/engine/fkv_shard/src/lib.rs`
  - `ShardState.store: Rc<Store>` → `stores: PartitionStoreMap`；读写 / 控制 /
    维护 / 归档 / attach / detach 全部按分区路由；`ensure_slot` 为「只需要
    IO + branch 身份、不打开表」的控制面请求（`ReopenExternal` /
    `ReopenArchive` / `DeleteArchive` / 归档列举）建槽。
  - 新增 `WriteTask` / `WriteFlight` / `WriteDone`：`spawn_put_group` 在任务内
    执行 group commit、OOM 重试、逐条回执、`record_request`、写后
    `maintain` / `drain_pending_ttl`；任务 panic 时丢槽位（后续按需 reopen）。
  - 主循环：每轮回收已完成写任务并 `gate.finish`；无就绪请求时 `wait_event`
    同时等待「新请求 / 写完成 / reopen 到期」；控制请求（Reopen / Attach /
    Detach / Shutdown）处理前 drain 全部在飞写；通道关闭 / Shutdown 同样先
    drain，避免半截落盘。
  - 读边界：目标分区正在写时，同分区读进 `blocked_read_li`，写完成后重试；
    其它分区读照常 `spawn`，不再等全局写结束。
  - `run_due_reopen` 跳过 Store 被写任务占用的分区，顺延一轮。
- `crate/engine/fkv_store/src/lib.rs`（最小接口，3 个方法）
  - `Store::adopt_attach`：新分区 Store 采纳 branch / term，不清共享 IO 缓存。
  - `Store::with_index_cache_ledger`：共享聚合字节账本（上限仍按缓存实例均分）。
  - `Store::term`：term 一致时避免重复 `set_term`（会清共享缓存）。

### 为什么不需要更大 Store 拆分

分区盘面本来就互不共享：main 分支每表独立 manifest，branch manifest 也在
`part_dir(tbl)` 下同名独立文件；`Store` 的 `snapshot` / `IndexCache` / TTL /
manifest 元数据都按表键控。把表状态拆成独立 `Store` 后，跨分区写之间没有
共享 `&mut`，也无需把 `Store` 改成内部可变或引入 dyn。剩下的 shard 级共享项
（IO 后端、index 缓存账本、term / branch 身份）都是只读或原子 / `Rc` 共享。

### 测试

新增 `crate/engine/fkv_shard/tests/write_concurrency.rs`（4 条）：

1. `cross_partition_writes_overlap`：两个分区各一条 4000 entry 慢写，独立
   线程采样 `write_running`，峰值 ≥ 2。
2. `max_write_concurrency_one_serializes_partitions`：同一场景 `= 1`，峰值恒 1。
3. `same_partition_writes_complete_in_fifo_order`：同分区 32 条写按入队顺序完成
   （回执顺序 = 完成顺序）。
4. `read_on_other_partition_progresses_during_slow_write`：大写在 A 进行中，
   B 的 `Get` 先完成。

### 验证

```
cargo nextest run -p fkv_shard                     # 59 passed（55 旧 + 4 新）
cargo nextest run -p fkv_shard --test write_concurrency  # 连续 15+ 轮全绿
cargo clippy -p fkv_shard --all-targets -- -D warnings    # 0 warning
cargo +nightly fmt -p fkv_shard -- --check                # 通过
cargo check -p fkv_store -p fkv_runtime                   # 通过
cargo nextest run -p fkv_runtime -E 'test(modes) or test(store_path) or test(maintain)'
cargo nextest run -p fkv_runtime --test options --test large_value
```

### 剩余 / 风险

1. **`PutLarge` / `GetLarge` 仍串行**：借用单个 `fixed_buffer`（io-07 池化
   未落地），跨分区大 value 写不并发；普通 `Put` 已并发。
2. **控制请求 drain 粒度保守**：Reopen / Attach / Detach / Shutdown 处理前
   等全部在飞写（而非仅同分区），语义安全但会多等无关分区。
3. **root meta 预算**：只在新建分区 Store 时按近似 LRU 收敛；单分区写增长
   期间不做全局复核（`Store` 内部单表也无法驱逐自己）。默认预算下与
   「每表可占满 shard 预算」的旧行为不同，已在 `partition.rs` 注释记档。
4. **写任务 panic**：该分区内存 Store 丢失，后续请求按需 reopen；panic 前
   已回执的写不受影响，未回执的调用方收到 `NotRunning`。正常路径不应触发。
5. **Cloud 后端并发写**：共享 `CloudStore` 的缓存状态只在同步段借用、未跨
   await 持锁；本轮测试覆盖 Local / Mem，Cloud 分区并发写未加专项压测。
6. **`sync_cache_metrics` 的 bytes gauge**：写任务进行中的分区用上次上报值，
   峰值口径略滞后（计数 hit/miss/evict 在任务完成后补报，不丢）。
7. **峰值内存指标**：`max_write_concurrency` 已是真正的派发额度，但「峰值内存
   随额度单调受限」的专项指标 / 压测仍归 `eng-32c`，本轮只保证不超发任务。

## 复审（2026-11-05 r7）

**判定：通过。** 逐条按 DONE 核对源码与测试，证据均亲读（复审时
`fkv_shard/src/lib.rs` 正被并行会话编辑、行号有漂移，以下 lib.rs 行号为复审
时点读数；结构本身稳定无冲突）：

- **partition.rs 属实**：`PartitionStoreMap` + `Slot { Option<Rc<ShardStore>>,
  cache_stat, touch }`，`take`（`Rc::try_unwrap` 失败放回回 `Busy`）/ `put_back` /
  `lose`（panic 丢槽位 + `backend.evict_table`）分列（`partition.rs:174-207`）；
  root meta 预算近似 LRU 只驱逐空闲分区（`Rc::strong_count == 1`，
  `:311-350`）；`sync_cache_metrics` 写中分区沿用上次 bytes 基线（`:258-275`），
  与「剩余 6」自述一致。
- **backend.rs 属实**：三变体 `Rc<…>` + `#[derive(Clone)]`（`backend.rs:78-86`）。
- **lib.rs 属实**：`WriteTask`/`WriteDone`/`WriteFlight`（`FuturesUnordered`，
  JoinHandle Err → `store: None`）；`spawn_put_group` 任务内 group commit、OOM
  重试、逐条回执 + `record_request`、写后 `maintain_store`；`drain_write_gate`
  高优先 Put 组 `take_pending_while(MAX_PUT_GROUP_REQUESTS-1)` 合并后派发；
  主循环 `try_next` 回收 + `promote_blocked_reads`；控制请求 / 通道 Closed 先
  `write_flight.drain`；`run_due_reopen` 对 `is_writing` 分区顺延一轮（不耗
  重试预算）。`fkv_store` 三方法在位：`adopt_attach`（`fkv_store/src/lib.rs:1825`，
  不清共享 IO 缓存）、`with_index_cache_ledger`（`:412`）、`term`（`:908`）。
- **测试抽读属实**：`tests/write_concurrency.rs` 4 条与清单一一对应——
  `cross_partition_writes_overlap`（`max_running >= 2` + 两分区数据互不覆盖 +
  收尾 `write_running == 0`）、`max_write_concurrency_one_serializes_partitions`
  （`assert_eq!(max_running, 1)`）、`same_partition_writes_complete_in_fifo_order`
  （32 条回执序 == 入队序）、`read_on_other_partition_progresses_during_slow_write`
  （`["get","put"]` 顺序断言）。
- **与 20-write-batching / eng-32 的交互（写并发正确性）**：合并在主循环串行段
  完成（`merge_put_batch_li` 引用排序单趟折叠 + `prepare_put_group` 的
  admit/term 校验），执行在任务内独占 `&mut Store`；同分区 FIFO 由
  `WriteGate::take_next` 跳过 `running` 分区保证；写派发前 `finish_read_li`
  drain 全部在飞读再 `Rc::try_unwrap`（保守但正确，`Busy` 为防御回执路径）；
  eng-32b 读 spawn 与写任务经 `blocked_read_li` 边界互斥；`PutLarge` 不入组
  （`put_term` 只匹配 `Put`），仍走 `run_gated_write` 串行——与「剩余 1」一致。
  未发现正确性回归。
- **验证声称**（nextest 59 passed 等）因并行会话占锁未复跑，以代码 + 测试文本
  核对为准。

**残余（非阻断，已立新 issue）**：`doc/architecture/06-gaps.md`、
`doc/architecture/02-execution-model.md`、`doc/migration.md`（M3/M4）共 4 处
仍写「跨分区写不并发 / 峰值写并发 1」，与本轮落地相反，且 06-gaps / 02 的
`issue/todo/eng-33a` 链接已失效 → 见 `issue/new/rev-48`（eng-33a DONE 未自述
此项）。
