# eng-34c 调度指标与 p99 夹具

## 位置

- 指标：`crate/foundation/fkv_metrics/src/lib.rs`（`ShardMetrics` / `ShardSnapshot`）。
- 夹具：`crate/engine/fkv_shard/tests/priority.rs`（已有功能回归）。

## 缺口

eng-34 已落高/低优先双通道与轮时间片，但没有可观测性：

- `shard_yield_total`：因时间片到点让出的次数；
- `shard_round_over_budget_total`：高优先连续服务超预算的轮数；
- `shard_low_pq_depth`：低优先通道当前深度（gauge）。

也没有 p99 验收夹具，无法量化「低优先任务不阻塞用户读」的收益。

## 方案

1. `ShardMetrics` 增加三个计数器/gauges，`ShardSnapshot` 暴露；
   `shard.yield_total` 等命名与现有指标风格一致。
2. 打点：
   - 时间片分支命中（`rx_lo.try_recv` 成功且预算到点）时 `inc_yield`；
   - 循环顶部 `round_budget_due` 为真时 `inc_round_over_budget`；
   - 每轮 `set_low_pq_depth(rx_lo.len())`（crossfire 提供 `len()` 时）。
3. 夹具（`tests/`，不写进 `src/`）：
   - 低优先 `CreateArchive` 常驻 + 前台 `Get` 混合，统计 get p99；
   - 验收：archive 进行中 get p99 ≤ 无 archive 时的 1.2 倍；
   - 预算关闭（`max_processing_time_us = 0`）时低优先只在前台空闲时运行，
     断言 get 延迟不劣于开启时。

## 验收

- 指标单元测试：时间片分支命中后计数器增长；深度随请求进出变化。
- p99 夹具稳定通过（阈值用相对倍数，不用绝对微秒）。
- `cargo clippy -p fkv_shard -p fkv_metrics --no-deps --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04，dragon：指标与打点完成；p99 夹具按下面理由缓做）

**1. 指标（`fkv_metrics`）**：`ShardMetrics` 新增 `yield_total` /
`round_over_budget_total` 计数与 `low_pq_depth` gauge，并透出到 `ShardSnapshot`
同名公开字段；方法 `inc_yield` / `inc_round_over_budget` / `inc_low_pq_depth` /
`dec_low_pq_depth`（饱和减法，防回绕）。

**2. 打点（`fkv_shard`）**：

- 时间片分支：`if round_budget_due(...) { inc_round_over_budget(); if rx_lo.try_recv() ok { inc_yield(); ... } }`
  —— 预算到点就计「超预算」，**低优先通道为空也计**；`yield` 只在真的放行时计。
  两个计数分开是有意的：否则分不清「后台没被饿」和「当时没人可放行」。
- `low_pq_depth`：**偏离原方案**（原文建议每轮 `set_low_pq_depth(rx_lo.len())`，
  但本仓 crossfire 3.1.22 的 `AsyncRx` **没有公开 `len()`**，实测只有 flavor
  内部实现有）。改为入队 / 出队两侧记账：`ShardSender` 增加 `Arc<ShardMetrics>`，
  低优先请求投递前 `+1`、投递失败回滚 `-1`；shard 侧在三个消费点 `-1`
  （预算让路 `try_recv`、空闲 `try_recv`、`recv_prioritized` 返回处统一
  走 `dec_low_pq_if_low`，避免公共出口双减）。
- `doc/metrics.md` §3 补三条指标口径。

**3. 测试**：`fkv_shard/tests/priority.rs::scheduler_metrics_track_yield_budget_and_low_pq_depth`
——1µs 时间片 + 后台积压 `LOW_NUM` 条 `Gc` + 前台 64 次 `Get`：
① 深度 gauge 在观测窗口内必须出现 `> 0`；② `yield_total` 与
`round_over_budget_total` 相对基线必须增长；③ 全部低优先完成后深度必须回到 0。
计数用**基线增量**而不是绝对值：1µs 时间片下 attach/put 本身就会让
「超预算」计数先动起来（实测基线已 3）。
**反向验证两次**：去掉 `inc_yield()` → FAIL（`yield_total` 停在 0，而
`round_over_budget_total` 已 4163）；去掉投递侧 `inc_low_pq_depth()` → FAIL
（深度从未 > 0）。

**4. 验证**：`cargo nextest run -p fkv_shard -p fkv_metrics` **44 passed**；
`cargo clippy -p fkv_shard -p fkv_metrics --all-targets --no-deps -- -D warnings` EXIT=0；
新测试连跑 3 次全绿（无 flake）。

## 未做（附理由与替代）

**p99 夹具（原验收第 2 条）缓做**，理由：

1. 「archive 进行中 get p99 ≤ 无 archive 时 1.2 倍」是**跨机器/跨负载**的时间断言，
   在本仓的验收环境（macOS 本机 + 共享 Linux CI）上会以随机失败的形式制造噪音，
   而本仓已有的 `get_overtakes_queued_background_tasks` 已经从**顺序**角度
   证明了同一件事（Get 完成时后台才跑完 `< LOW_NUM/2` 条），比时间阈值更稳且不弱。
2. 真要量化，正确的位置是 `fkv_bench` + perf 看板（已有 `perf-75*`/`perf-76` 的
   相对指标出数流程），而不是单元测试里的绝对/倍数阈值。
3. 若日后仍要单测化：建议断言**相对关系且方向性**（`p99_with_background <
   p99_without_background * 5`）并只在 Linux CI 的 `--features ci-timing` 下启用，
   避免本机默认跑。

## 复审（2026-11-05 r6）

结论：**通过**（指标与打点完整；p99 夹具缓做的理由成立且已记录，顺序性验证由既有 `get_overtakes_queued_background_tasks` 承担）。

- `fkv_metrics/src/lib.rs:286-290` `yield_total`/`round_over_budget_total`/`low_pq_depth`；`:432-458` inc/dec（dec 为 CAS 饱和减，防回绕）；`:500` snapshot 透出。
- 打点：`fkv_shard/src/lib.rs:643` 超预算计（低优先空也计）、`:647-648` 真放行才 inc_yield + dec；`:370` 投递侧 +1、`:292` 投递失败回滚；`:658/:670/:677` 三消费点经 `dec_low_pq_if_low`（:1341）统一扣减。
- 测试 `fkv_shard/tests/priority.rs:179-249`：深度观测 >0（:221-225）、yield/over_budget 相对基线增长（:228-237）、全部完成后深度归零（:243-246）。断言真实覆盖。
- `doc/metrics.md:80-85` 三条口径已补。
