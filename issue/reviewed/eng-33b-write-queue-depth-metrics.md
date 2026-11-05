# eng-33b 写队列深度与在跑写指标

父 issue：`eng-33-write-gate-missing.md`（已落地串行形态，见 `issue/done/`）。
与 `eng-32c` 的「ready 队列深度 / in-flight 指标」同批实施，不单独排期。

## 现状

- `fkv_shard/src/write_gate.rs` 的 `pending_len` / `running_len` 目前只有
  `#[cfg(test)]`，生产侧没有队列深度可观测点。
- `ShardMetrics::observe` 只记延迟/错误，无 gauge（`eng-32c` 已记录同一缺口）。
- 串行 work loop 下 `pending` 恒为 0（队列在 crossfire channel 里），指标要等
  `eng-33a` 有真正排队后才有观测价值。

## 方案

- `fkv_metrics` 增加 `shard_write_queue_depth`、`shard_write_running` gauge。
- 更新点：`WriteGate::push` / `take_next` / `finish` / `remove_pending`；shard 内独占，
  用 `Cell<usize>` 或 `AtomicUsize`（Relaxed）即可，避免锁。
- 指标只在 shard 线程写，读侧从 `Shard::metrics_snapshot` 取，不加全局同步开销。

## 验收

- 集成测试断言：`max_write_concurrency = 1` + N 条同分区写时，`running` 最大为 1；
  `> 1` 且跨分区时 `running` 可 > 1（依赖 `eng-33a`）。
- 指标可读且与 `eng-32c` 的 in-flight / ready 深度口径一致。

## DONE（2026-11-04）

队列深度 / 在跑写已进入 shard 指标，改动如下：

- `fkv_metrics`：`ShardMetrics` 新增 `write_queue_depth` / `write_running` 两个
  `AtomicU64` gauge（`set_write_queue_depth` / `set_write_running`），并透出到
  `ShardSnapshot.write_queue_depth` / `write_running`，口径与现有
  `set_index_cache_bytes` 一致（shard 线程写，Runtime 线程 `metrics_snapshot` 读）。
- `fkv_shard/src/write_gate.rs`：`WriteGate` 持有 `Arc<ShardMetrics>`，新增跨分区
  排队总数 `pending`（增量维护，`publish` 为 O(1)，不做全表求和）。在
  `push` / `take_next`（弹出成功时） / `finish` / `remove_pending` / `clear`
  之后各调一次 `publish()` 上报；额度已满返回 `None` 的路径状态未变，不上报。
  `pending_len` / `running_len` 保留为测试断言用（`#[cfg(test)]`）。
- `fkv_shard/src/lib.rs`：`WriteGate::new(config.max_write_concurrency, thread_metrics.clone())`，
  未改 work loop 语义。
- `doc/metrics.md`：补记两个 gauge，并把 `task_manager_active_tasks` 一行标记为
  写队列侧已上报、读侧 ready 深度待 `eng-32c`。

验收对应：

- 单测（`write_gate::test`）逐点断言 gauge：push 后深度=N 在跑=0；`take_next`
  后深度-1 在跑=1；同分区第二条不可启动时在跑恒 1；`max_concurrency=1` 在跑恒 ≤1；
  `remove_pending` / `clear` 后深度归零。
- 集成测试 `tests/write_gate.rs::metrics_track_running_write_and_queue_depth`：
  `max_write_concurrency=1` + 同分区 16 条写，独立 OS 线程高频采样，断言在跑写
  峰值恰为 1、结束后 `write_running=0` / `write_queue_depth=0`。
- `cargo nextest run -p fkv_shard` 全绿（27 passed），
  `cargo clippy -p fkv_shard --all-targets --no-deps -- -D warnings` 零警告
  （`fkv_metrics` 同）。

遗留：`running > 1` 的并发观测依赖 `eng-33a`（当前 work loop 串行，`running`
恒 ≤1）；串行形态下 `pending` 基本为 0（写请求积压在 crossfire 通道而非 gate），
gauge 的深度语义已由单测覆盖。

## 复审（2026-11-05 r6）

结论：**通过**。

- `fkv_metrics/src/lib.rs:282-284` `write_queue_depth`/`write_running` AtomicU64 gauge；`:421-427` setter；`:498-499` 透出 ShardSnapshot；`:534-536` snapshot 字段。
- `fkv_shard/src/write_gate.rs:49` 持 `Arc<ShardMetrics>`；`:66-73` `publish()` O(1) 增量上报，在 push(:85)/take_next(:108)/take_pending_while(:142)/finish(:163)/remove_pending(:179)/clear(:193) 后各调一次。
- 单测断言 gauge：`same_partition_is_fifo_and_exclusive`（write_gate.rs:236-252 深度/在跑逐点断言）、`remove_pending_and_clear`（:333-339 深度归零）。
- `doc/metrics.md:77-78,106` 已补记两个 gauge。
- 遗留（running>1 依赖 eng-33a）与 md 记录一致。
