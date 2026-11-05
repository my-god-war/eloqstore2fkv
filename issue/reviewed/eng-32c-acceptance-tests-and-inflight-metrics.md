# eng-32c 并发读验收用例与 in-flight / 队列深度指标

父 issue：`eng-32-workloop-serialization.md`。内容来自 `03-shard-request-concurrency.md`
的独有部分（验收 + 指标），不单独排期。

## 集成测试（`crate/engine/fkv_shard/tests/`）

1. 32 并发 `Get`：单 shard 一次收 32 条读，全部结果正确，无 panic。
2. 慢表 `Scan` 与另一表 `Get` 同时排队：`Get` 不受 `Scan` 阻塞（读批上限内的并发）。
3. attach/detach 与并发请求线性化：detach 后的请求返回 `NotRunning`，在飞读全部完成回执。

## 指标（`fkv_metrics`）

- `ShardMetrics` 增加 in-flight gauge（当前在读/写请求数）。
- ready 队列深度、读批大小直方图。
- 每请求 IO 深度（配合 `MeteredIo` 的 in-flight 计数）。

## 现状

`ShardMetrics::observe` 只记延迟/错误（`crate/foundation/fkv_metrics/src/lib.rs:257`），
无队列深度与 in-flight 字段；`max_write_concurrency` 在 `fkv_config` 未接线（`eng-56`）。

## 验收

指标可读且与并发读测试一致；测试进 CI。

## DONE（2026-11-05）

### 状态

读侧 in-flight / 读批 / IO 深度指标已接入，eng-32c 的四条验收测试落地。
写侧复用 `eng-33b` 的 `write_queue_depth` / `write_running`，本次未改写门语义。
`fkv_runtime` 无需改动：`MetricsSnapshot.shard_li` 直接透出新增的
`ShardSnapshot` 字段。

### 指标（`fkv_metrics`）

- `ShardSnapshot` 新增：
  - `read_in_flight`：当前在飞读任务数（gauge）；
  - `read_in_flight_max`：读在飞峰值（进程内单调）；
  - `read_batch_size`：读批大小直方图（`ReadBatchSnapshot`），桶
    `READ_BATCH_UPPER = [1, 2, 4, 8, 16, 32]` + `+Inf`，计数口径，不复用延迟桶；
  - `io_in_flight` / `io_in_flight_max`：`MeteredIo` 对所有异步 IO 的在飞计数
    与峰值（read / write / manifest / file / segment 全部计入）。
- `MeteredIo` 的每个异步 IO 经 `track()` + RAII `IoFlightGuard` 进出计数，
  覆盖错误返回与 future 取消，不泄漏计数。
- `ShardMetric` 新增 `set_read_in_flight` / `record_read_batch`；空批不记。

### shard 接线（`fkv_shard`）

- `ReadFlight` 持 `Arc<ShardMetric>`，`push` / `record` / `drain` /
  `finish_read_li` 后上报 `read_in_flight`；以 `task_set.len()` 为唯一在飞口径。
- `submit_read_li` 记录本轮合并提交的读请求批大小。
- `MAX_READ_BATCH` 由私有改 `pub`，作为测试与文档的唯一上限来源。

### 验收测试

新增 `crate/engine/fkv_shard/tests/read_concurrency.rs`（4 条）：

1. `thirty_two_concurrent_gets_are_served_and_gauged`：32 并发 `Get` 全部返回
   正确值；`read_in_flight` 归零，峰值与读批不超过 `MAX_READ_BATCH`。
2. `slow_scan_does_not_block_other_table_get`：512 entry 慢表 `Scan` 与另一表
   16 条 `Get` 同窗口并发；`Get` 5s 内完成，`read_in_flight_max >= 2`（两个读
   任务确实同时在飞），`io_in_flight_max >= 1`。
3. `detach_linearizes_inflight_reads`：detach 前入队的 96 条读全部回执（值正确
   或 `NotRunning`），detach 后新读 `NotRunning`，重 attach 后恢复；gauge 归零。
4. `read_in_flight_peak_is_bounded_by_batch_limit`：慢 scan + 96 条突发读，
   在飞峰值与读批均 ≤ `MAX_READ_BATCH`，全部值正确。

另补：

- `fkv_metrics/tests/metrics.rs`：读写 gauge 的当前值 / 峰值、读批直方图桶、
  `MeteredIo` 4 个并发 IO 的在飞深度（测试 PageIo 固定延时，确定性观测）。
- `fkv_shard/src/lib.rs` 单测：`read_flight_holds_store_until_drained` 增加
  gauge 断言；新增 `read_flight_is_full_at_batch_limit`（第 32 条后 `is_full`，
  峰值等于上限，drain 后归零）。

### 验证

```
cargo nextest run -p fkv_metrics                          # 15 passed
cargo clippy -p fkv_metrics --all-targets -- -D warnings  # 0 warning
cargo nextest run -p fkv_shard --test read_concurrency    # 4 passed（连续 8 轮）
cargo nextest run -p fkv_shard --lib -E 'test(read_flight)' # 2 passed
cargo clippy -p fkv_shard --all-targets -- -D warnings    # 0 warning
cargo nextest run -p fkv_shard                            # 75 passed
cargo check -p fkv_runtime                                # 通过
cargo +nightly fmt --check（改动文件）                     # 通过
```

`fkv_shard` 全量 75 条全部通过（含本条新增的 4 条验收 + 2 条 `read_flight`
单测）；写侧 `write_concurrency` / `write_gate` 的 gauge 用例未受影响。

### 风险 / 边界

1. `read_in_flight` 只统计 spawn 的读任务；`GetLarge` / `PutLarge` 串行路径
   不计入。
2. `read_batch_size` 记录的是 `submit_read_li` 收集到的批大小（含随后被取消 /
   超时过滤的条目）；读窗口内逐条准入的读不计入。
3. `io_in_flight` 是 shard 级 IO 并发总数，不是单请求精确深度；单读任务独占时
   可视为该请求深度，多请求并发时用 `read_in_flight` 均摊。
4. `read_in_flight_max` / `io_in_flight_max` 是进程内峰值，不随窗口结束重置。
5. `MeteredIo::track()` 给每次异步 IO 多包一层 future；单 shard 单线程下开销
   可忽略，未做专项基准对比。

## 复审（2026-11-05 r7）

**判定：通过，移入 reviewed/。** 逐项亲读源码与测试（未运行 cargo，按本仓复审口径以在盘代码为准）：

- 指标（`fkv_metrics/src/lib.rs`）：`READ_BATCH_UPPER=[1,2,4,8,16,32]`（:38）、
  `ReadBatchSnapshot`（:289）、gauge/峰值字段（:370-378）、`set_read_in_flight`
  /`record_read_batch`（:530/:538）、`ShardSnapshot` 透出四字段 + `read_batch_size`
  （:670-678）；`MeteredIo` 对 PageIo / ManifestIo / FileIo / SegmentIo /
  FixedSegmentIo / TermIo 的全部异步方法包 `track()`（:755-1160），`IoFlightGuard`
  RAII 进出计数（:738-744）——「全部计入」声称成立。
- shard 接线：`MAX_READ_BATCH` pub（fkv_shard/src/lib.rs:64）；`ReadFlight` 持
  `Arc<ShardMetric>`，drain 后按 `task_set.len()` 上报（:2480）；`submit_read_li`
  记批大小（:2554）。
- 验收测试 4 条名实相符（tests/read_concurrency.rs:142/189/250/304），关键断言
  亲读：32 并发后 gauge 归零、批与峰值 ≤ 上限（:167-180）；scan+get 并发
  `read_in_flight_max>=2 && io_in_flight_max>=1`（:231-241）；metrics.rs 峰值 /
  4 并发 IO 深度（tests/metrics.rs:145-153/:244-250）；lib 单测两条在位
  （:3368/:3422，第 32 条 `is_full`、drain 后归零 :3447-3448）。
- 风险段 5 条自述与代码一致（submit 口径批大小、进程内峰值不重置等）。
- 登记：06-gaps.md:76-78 已收录；migration.md M2 行仍列「缺读侧 in-flight /
  读批指标」→ rev-49 承接。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 本轮按 DONE 声称逐项亲读在盘代码（未运行 cargo）：

- 指标（`fkv_metrics/src/lib.rs`，行号为本轮亲读）：`READ_BATCH_UPPER=[1,2,4,8,16,32]`
  （:38）、`ReadBatchSnapshot`（:289）、`ShardMetric` 五字段 `read_in_flight` /
  `_max` / `read_batch_size` / `io_in_flight` / `io_in_flight_max`（:370-378）、
  `set_read_in_flight`（:530）、`record_read_batch`（:538）、`inc/dec_io_in_flight`
  （:545/:553）、`ShardSnapshot` 透出（:627-631、:670-678）、`MeteredIo::track`
  包 future（:731）、`IoFlightGuard` RAII（:738-751）——「全部计入 + 不泄漏」成立。
- shard 接线（`fkv_shard/src/lib.rs`）：`MAX_READ_BATCH` 已 pub（:65）、`ReadFlight`
  （:2516）、drain 后 `set_read_in_flight(task_set.len())`（:2540）、`submit_read_li`
  记批大小（:2614）、单测 `read_flight_is_full_at_batch_limit`（:3556，:3584-3585
  断言峰值==上限、归零）。
- 验收测试 4 条在位（`tests/read_concurrency.rs:143/190/251/305`），关键断言亲读：
  32 并发后 gauge 归零、峰值/读批 ≤ 上限（:168-180）；scan+get 同窗并发
  `read_in_flight_max>=2 && io_in_flight_max>=1`（:233-241）；detach 线性化 + gauge
  归零（:291-292）；96 突发 < 批上限（:346-354）。metrics.rs 三条在位
  （`read_in_flight_gauge_tracks_current_and_peak` :146、`read_batch_histogram_...`
  :159、`metered_io_tracks_in_flight_depth_and_peak` :222，4 并发 IO 深度断言
  :245-246/:251）。
- 风险段 5 条自述与代码一致（submit 口径批大小、进程内峰值不重置、GetLarge/PutLarge
  串行不计入）。行号相对 DONE 有漂移（并行会话改 `fkv_shard/src/lib.rs`），以内容锚点
  为准；指标与测试文件（05:17/05:48）不在写入区，锚点稳定。

