# test-24 `queue_backpressure::dropped_pending_low_request_rolls_back_depth` 极端负载下间歇性真挂起（0% CPU 双侧空闲 ≥8 分钟），同期 `shutdown_race` 15s 守卫再现——负载敏感既有 flaky 首次连续阻塞全量门禁

- 领域：测试质量（负载敏感 flaky × 低优先队列可能的丢失唤醒窗口，建议异步域会签）
- 优先级：P3（同形 shard 代码第 32 轮全量绿；本轮五提交均未触碰 fkv_shard
  及其依赖行为，非回归；但真挂起（区别于慢超时）一旦落在门禁上会以 120s
  硬超时 ×N 次的形态阻塞收敛判定，且 0b87909 上同样复现实证「既有」）
- 置信度：现象高（6 连复现 + 采样 + 跨提交 + 第三检出佐证 + 消退对照）；
  根因未定位（静态采样只能到「双侧空闲」，未做 runtime 级 trace）

## 现象（第 33 轮结构域门禁全量，fkv `4c40a75`，darwin arm64 12 核，当晚 load 6~12.7 持续）

1. 全量 pass1：`1543 passed / 1 timed out / 1 skipped`——
   `fkv_shard::queue_backpressure::dropped_pending_low_request_rolls_back_depth`
   nextest 120s 硬超时；
2. 单测隔离复跑 ×2：仍 120s 整超时；
3. libtest 直跑 + 外部 `timeout 480`：480s 未完成，全程 **0.08s user CPU**
   （真挂起，非 IO 慢）；
4. 采样（`sample` 3s ×2，存档 `/tmp/r29/qbp_sample.txt` / `qbp_sample2.txt`）：
   测试 future park 在 compio Proactor `kevent`；`fkv-shard` 事件循环线程
   park 在 compio `asyncify` 的 flume `recv_sync`（`park_timeout`）；9 个
   worker 线程全部 `dispatch_semaphore` 空闲——**无任何可运行线程**，而此时
   在途的有 1 条全表 Scan + 512 条 CreateArchive（queue_size=1 前提成立，
   挂起发生在前提断言之后）；
5. **归属实验**：同一用例在 `0b87909`（第 32 轮验证过全绿的同 shard 代码）
   同机同样 120s 超时——非本轮 `1b064a4/7179442/a0b076c/1a48b8c/4c40a75`
   回归（五提交不触碰 fkv_shard）；
6. 第三检出佐证：`/Users/z/.jcode/scratch/fkv_clean_target` 的
   `queue_backpressure-ca88e258936b02ab` 进程自当晚 20:02 起悬挂 ~2h
   （另一 agent 的 `cargo test` 残留），说明挂起在本轮门禁开始前已在发生；
7. 全量 pass3（~22:12，负载回落后）：backpressure **通过**，但
   `fkv_shard::shutdown_race::requests_racing_shutdown_all_resolve` 触发
   其内部 15s 互等守卫（test-05 锁）——第 14 轮先例（load 85 单次环境性
   超时、单跑 10/10 过、判环境性不立项）的同形再现；随后两用例隔离复跑
   均秒过（0.042s / 0.470s）。

当晚机器背景：UTM VM agent（bootHarness + vm-mcp）+ 系统态异常
（opendirectoryd 187% CPU、Spotlight 索引风暴）+ 并行 bench agent
（fkv-rel 上 fkv_bench 100% CPU），12 核持续 load ~10。

## 与第 14 轮先例的区别

第 14 轮 shutdown_race 是 **CPU 饿死导致的慢超时**（15s 守卫、有 CPU 活动）；
本轮 backpressure 是 **0% CPU 双侧空闲的真挂起**（丢失唤醒形态），且 6 连
复现横跨两个提交与第三个检出。二者的测试语义相邻（都在 shard 通道唤醒 /
关停时序面上），不排除同一底层窗口的两种显形。

## 建议

1. 异步域会签：确认「低优先通道满 + 调用方以 noop_waker poll 一次后 drop」
   的交错下，通道唤醒是否存在丢失窗口（test-05 曾实证过 shutdown 死锁并
   修复，唤醒路径有前科）；若坐实引擎侧丢失唤醒则转语义域立项。
2. 若判定纯环境触发（负载改变夹具时序使挂起显形）：测试侧加内部等待上限
   （复用 shutdown_race 的 15s 守卫模式）使挂起快速失败而非 120s 硬超时
   悬挂，或标注重载机器隔离跑法。
3. 复现材料：`/tmp/r29/qbp_sample.txt`、`/tmp/r29/qbp_sample2.txt`
   （两份 3s 采样全栈）、`/tmp/r29/clippy_r33.log` 同期环境。

## 验收方式

1. 极端负载下连续 ≥3 次全量该用例不再 120s 硬超时（或挂起转守卫快速失败）；
2. 异步域对唤醒窗口的会签结论登记（引擎缺陷转语义立项，或环境性盖章）。

（发现轮：第 33 轮结构 / 测试域门禁全量；判为本域新发现——第 14 轮先例
仅覆盖 shutdown_race 的慢超时形态，未覆盖 backpressure 的真挂起形态。）

## 实现记录（2026-11-07，含异步专家会签与协调者对照实验）

- **fkv commit `ebb356e`**（仅 test 面）：用例加 15s 限时守卫
  （`compio::time::timeout(SCENE_DEADLINE, drop_pending_scene())`，超时
  panic 给诊断而非 120s 硬超时悬挂门禁）+ 文件头「已知构造性风险（非引擎
  缺陷）」说明（crossfire 取消协议 + 复现条件）。
- **会签结论（协调者对照实验，异步面）**：
  1. 触发条件**分离**：同场景去掉「poll 一次即 drop」构造（其余不变）
     连跑 3 次全通过 ⇒ 挂起需取消 send future，非通道自发丢失唤醒；
  2. 挂起点定位（带标记的探针副本，跑后已删）：卡在某个 backlog
     `handle.await`（`backlog 256` 之后），即某条低优先发送被滞留；
  3. 复现率：本机带并行构建负载下 **5/6**（守卫 15s 处失败）——
     高于第 33 轮审查时的间歇形态，机器负载敏感；
  4. crossfire 3.1.22 对该交错有**显式处理**（`SendFuture::drop` →
     `abandon_send_waker` 的 `Woken` 分支调 `on_recv()`），但缺口未逐行
     定位 ⇒ 根因未定案，转 **io-32**（P2）承接收敛。
- **生产面**：任何在入队等待中被取消的请求都会走该路径；默认
  `queue_timeout = None`（无界）⇒ 滞留请求永不回执（io-32 建议 2）。
- 结论：test-24 的「测试侧」处置（守卫 + 风险说明）即本条验收；引擎侧
  修复/根因由 io-32 跟踪。

## 复审记录（2026-11-08，第 35 轮结构域）

- 第 34/35 轮判「不达标」的唯一成因是 `ebb356e` 守卫行触发 `absolute_paths`
  门禁（→ struct-45）——已修（`52650d6`）。
- 本条自身的交付（15s 守卫 + 风险头注）已到位；且**其刻画的现象已在第 35 轮
  由 io-32 定案并修复**（事件循环 park 判据忽略写门，`6524c4b`）：修复后
  `queue_backpressure` 在负载 load≈33 下 **10/10 通过、0 次守卫触发**，
  冻结快照再跑 8/8（~0.5s）且全量 1505/1505。守卫按 io-32 建议保留为廉价保险
  （预期永不触发）。⇒ 移 reviewed/。
