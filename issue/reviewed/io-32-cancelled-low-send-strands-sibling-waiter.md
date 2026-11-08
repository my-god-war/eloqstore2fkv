# io-32 低优先 CreateArchive 洪峰 + 在读全表 Scan 下 shard 停止回执：dequeue 531 / reply 45，通道与 crossfire send registry 均空、事件循环停在 kevent（0% CPU 双侧空闲 ≥15s）

- 领域：异步 / IO 模型（shard 事件循环 × 写门 / 读窗 × 错误路径资源归还）
- 优先级：**P1**（引擎级 liveness：请求被 dequeue 后永不回执，且 event loop
  就地 park；生产形态为「低优先维护洪峰 + 并发全表读」，非测试专有）
- 置信度：现象与归属**高**（crossfire 探针 + shard 埋点运行时取证，见 §取证）；
  根因**未定**（三条候选线索已备好埋点配方，见 §下一步）

## 结论（2026-11-08 取证后改判）

- **原假设（取消 send future 滞留兄弟等待者）已被否证**：失败运行中低优先通道
  `push 513 / pop 513`、末态通道空、crossfire send registry 归零
  （`FIRE begin queue_len=0`）——**不存在滞留的发送者**；最小库级复现
  （honor-waker 真实执行器 + 取消发送者，~150 轮）0 滞留。
- **真实现象与归属**：512 条低优先 `CreateArchive` 全部入队并被 shard **取走**
  （dequeue 531），但 **reply 只有 45**；随后 shard 事件循环 park 在
  `Runtime::block_on → Proactor::poll → kevent`（全机空闲、0% CPU）直到 15s
  守卫——即**引擎消费了请求却不回执，并把事件循环停死**。请愿上界（如
  `queue_timeout`）**不解决**该形态（send 已成功）。
- 一条强线索：R1 失败运行的**最后一次 reply 是 `op=Some(CreateArchive) ok=false`**
  （错误回执）且紧邻停滞点 ⇒ 疑似**错误路径未归还写门 / 分区名额**，使后续
  请求全部卡在无法就绪的 gate 后面，事件循环再无可等待事件而 park。
- 本条目由 test-24 悬念升级而来；`issue/done/test-24-*.md` 的 15s 守卫与风险
  头注仍有效（守卫把悬挂变快速失败 + 诊断）。

## 取证（决定性，2026-11-08）

工具：给 crossfire 3.1.22 打 `CFX_LOG` 门控探针（逐实例 id：channel pos 迁移 /
`RegistryMulti` 的 reg/fire/abandon/wake / 各 flavor）并以 `[patch.crates-io]`
覆盖到 fkv worktree；shard 内埋 `FKV_DBG`（dequeue / reply）。载体
`fkv_shard::queue_backpressure::dropped_pending_low_request_rolls_back_depth`。
（探针与埋点在跑后已逐字节还原，worktree `diff -q` 校验一致。）

| 观测 | 失败 run 实测 |
|---|---|
| 低优先通道（`bounded_async(Envelope,1)` → `One` flavor） | `push 513 / pop 513`，末态 head==tail（**空**） |
| crossfire send registry 末态 | `FIRE begin queue_len=0 state=0`（**无滞留发送者**） |
| shard dequeue / reply | `531 / 45`（S3）、`253 / 118`（R1） |
| 测试完成的 backlog handle | `1/512`（S3）、`94/512`（R1） |
| `sample` 两次 3s | shard 线程停在 kevent；其余线程全空闲 |

日志：`/tmp/r29/io32/S3.txt`、`R1.txt`、`Q2.txt`、`k2.txt`、
`hang_sample_1.txt`；取证模板 `/tmp/r29/io32-cfx`；最小复现工程
`/tmp/r29/io32-repro`；用例原文快照 `/tmp/r29/io32/qbp_pristine.rs`。

**排除清单（crossfire 侧，逐项）**：`fire()` 游标语义 / `_clear_wakers` 早退 /
`reg_waker_async` 的 `will_wake` 早返 / `Woken` 分支未交接——在失败样本中均
未导致发送者滞留（registry 归零 + 通道归零 + push/pop 计数相等）。次级现象
（`R6 rx: FIRE begin queue_len=0`、`W seq=54 ABANDON -> Err(3)` 后补偿落空
registry）在**通过**的 run 中同样出现，主循环顶部 `try_recv` /
`sender_double_check` / `try_recv_final` 兜底重检 ⇒ **非致命**。

## 下一步（根因取证配方，已备）

在 shard 内埋点（或复用 `/tmp/r29/io32-cfx` 模板）观察：
1. `drain_write_gate` / `gate.has_ready()` 的进展（停滞时 gate 内是否仍有
   pending、是否无人唤醒）；
2. `write_flight` 在飞数 vs `max_write_concurrency`（本测试 config 的
   `max_inflight_write`）——是否存在「名额被占用而占用者已不再前进」；
3. **errored `CreateArchive`（`ok=false`）是否漏放分区写名额 / gate 槽位**
   （R1 线索，优先查）；
4. `event_loop.rs` 中 read 侧等待（`read_flight` / `large_park_li` /
   `read_window`）与低优先写队列的互锁：同分区长 Scan 在读窗内时低优先
   `CreateArchive` 的排空条件是否可能永不满足。

## 处置建议

1. **修复放在引擎**：定位并修复「dequeue 后不回执」的路径（首选线索 3）；
   修好后把 test-24 的取消路径覆盖恢复为无守卫形态（守卫保留为廉价保险）。
2. **等待上界（独立登记项，非本挂起的修复）**：`queue_timeout` 有界默认
   **不解决**本形态；若要有界「请求永不回执」，旋钮是整请求截止
   `ShardSender::request_timeout`（本端 deadline 兜底 + shard 侧 `expired()`）。
   两者都属 Rust 单侧加固（C++ 侧无请求级超时、无队列容量，语义会签见
   `issue/new/00-index.md` 第 34 轮语义段），另立条目并对照 C++ 后落地。
3. 现状（已完成）：test-24 15s 守卫 + 风险头注（fkv 提交见 test-24 实现记录；
   措辞已按 struct-45 收窄为候选）。

## 验收方式

1. 新条目（shard 停回执）定位并修复后，重载机器（12 核 load ≥ 20）下该用例
   连续 ≥3 次全量通过（无需守卫触发）；
2. 修复附「errored CreateArchive 名额归还」的注入式用例（若线索 3 成立）；
3. 背压/超时上界若落地，另附其对拍 C++ 的登记条目。

---

## 附录：原假设与证伪（保留为负证据，勿再重复）
- 领域：异步 / IO 模型（入队通道 × future 取消 × 唤醒协议）
- 优先级：P2（需「通道满 + 一个 send 被取消 + 其它等待者」交错：高负载下
  实测复现率 5/6；后果是**单个请求永不回执**（车 shard 侧空闲、通道空）。
  触发面真实存在于生产：任何在「入队等待中」被取消的请求（调用方超时 /
  任务取消 / crontab 停机）都会 drop 其 send future）
- 置信度：现象高（复现 + 对照 + 跨提交）；根因中（crossfire 3.1.22 对该
  交错有显式处理路径，缺口未精确定位到行）

## 现象与证据（fkv `4c40a75`，darwin arm64）

1. **触发点**：`fkv_shard/tests/queue_backpressure.rs` 的
   `dropped_pending_low_request_rolls_back_depth`——刻意构造「容量 1 的
   低优先通道满 + 512 个真实等待发送者 + 一个以 noop waker poll 一次即
   drop 的发送者」。
2. **复现**（本机带并行构建负载）：加 15s 限时守卫后连跑 6 次，**5 次在
   15s 守卫处失败**（此前为间歇；`0b87909` 同形复现 ⇒ 跨提交既有）。
3. **定位到挂起点**（带 `eprintln!` 标记的探针副本，跑后已删）：测试通过
   `scan done` → `backlog 0/64/128/192/256` 后停住——**卡在某个 backlog
   `handle.await`**，即某条低优先发送被滞留。
4. **对照实验**：同一场景**去掉 poll-once-drop**（其余不变）连跑 3 次
   全部通过（`backlog done`）⇒ 取消那个 send future 是触发条件，不是通道
   自发的丢失唤醒。
5. 早期采样（第 33 轮结构域）：测试 future park 于 compio `kevent`，
   shard 事件循环 park 于 asyncify `flume recv_sync`，worker 全空闲
   ——**无任何可运行线程**；与「发送者未入队 + shard 通道为空」一致。

## 机理候选（未定案）

crossfire 3.1.22 对「已注册 waker 被先唤醒（Waiting→Woken）后才 drop」有
**显式处理**：`async_tx.rs::SendFuture::drop` → `shared::abandon_send_waker`
在 `WakerState::Woken` 分支调用 `on_recv()`（注释：*We are awake, but give
up sending, should notify another sender for safety*），意图把这次唤醒转交
给下一个发送者。因此缺口在下列之一（未逐行定位）：

- `senders.fire()` 的迭代/游标在该分支下未真正唤醒下一个 `Waiting`
  发送者（registry 的 `Skip`（is_done=false，应继续迭代）与 `Woken`
  （停止）语义交界的边角）；
- 或 `reg_waker_async` 重注册时对 `Woken` 状态 waker 的处理（被唤醒的
  真实发送者在抢槽失败后重新注册，若状态未复位则后续唤醒全被跳过）；
- 或 `send_with_timer`（本仓低优先路径在有 queue_timeout 时走该变体）与
  `send` 的不同注册路径。

## 影响

- **生产 liveness**：`ShardSender::queue_timeout` 默认 `None`（
  `lifecycle.rs:515`；配置 `request_queue_timeout_us = 0` ⇒ None）——
  入队等待无上界；一条被滞留的请求（及其调用方）永不回执。低优先请求
  （Gc / Compact / CreateArchive …）由维护路径发出，滞留会让该维护步骤
  永久卡住；高优先请求同理（若队列满时被取消）。
- **门禁**：该用例在高负载下由 120s 硬超时悬挂变为 15s 快速失败（守卫
  已随本轮的 `queue_backpressure.rs` 提交落地），但高负载机器上仍是红。

## 建议处置

1. **短期（已落地）**：用例加 15s 限时守卫 + 文件头风险说明（fkv commit
   见实现记录），把悬挂变为快速失败 + 诊断。
2. **引擎侧加固候选**（需语义域会签）：给 `ShardSender` /
   `ShardConfig` 的 `queue_timeout` 设一个**有界默认**（如 5-10s，超时
   回 `Busy`），使任何入队滞留都有上界；代价是持续背压下由「静默挂起」
   变为「显式 Busy」——与 C++ 的背压语义需对照（C++ 是否有等价上界）。
3. **根因定案候选**：用 crossfire 3.1.22 仓库自带的测试设施或最小
   repro（容量 1 通道 + N 个真实等待 + 一个 poll 一次即 drop 的发送者，
   消费侧缓慢 drain）在**库评论级**定案，然后：若上游有修复版本则升级
   `crossfire`（当前 pin `3.1.22`）；否则向上游提交 issue。
4. 修好后把用例的取消路径覆盖恢复为无守卫形态（保留守卫为廉价保险）。

## 验收方式

1. 极端负载下该用例连续 ≥3 次全量不再守卫失败（或滞留的上界被显式
   `Busy` 取代后，用例断言不再需要守卫）；
2. 若走建议 2：新增「入队滞留超时的上界」用例（注入式），并登记与
   C++ 背压语义的差异/一致性；
3. 若走建议 3：crossfire 侧定案 + 版本/上游 issue 链接入本条目。

（发现轮：第 33 轮结构域立 test-24（现象：真挂起）；协调者在流转中
做对照/探针实验，分离出「取消 send future」为触发条件并确定滞留对象
为低优先发送者，故从测试质量面升级为本条异步面缺陷。）

## 最小复现尝试（负面结果，2026-11-08）

单线程「手写 poll 循环」模型（容量 1 的 `mpmc::bounded_async` + 8/512 个
真实 send 等待者 + 一个 poll 一次即 drop 的发送者 + 消费侧 try_recv 缓慢
drain）**未复现**（8 senders × 200 轮、512 senders × 100 轮均 0 滞留）。
原因：该模型**忽略 waker 语义**（每轮无条件重 poll 所有发送者），丢失唤醒
被「轮询兜底」掩盖；引擎用例跑在 compio 执行器上，task 只在被唤醒时重
poll，因此能显形。⇒ 忠实的最小复现需带真实执行器（honor waker 的
spawn + 逐个 task 挂起），留给上游报送时做；本条以引擎用例
（`queue_backpressure.rs` ± 守卫）为复现载体。

## 实现记录（2026-11-08，根因定案 + 修复，fkv `6524c4b`）

- **根因（定案）**：事件循环 park 判据（`event_loop.rs:216`）只查
  `read_flight` / `large_park_li`，**忽略写门**。`drain_write_gate` 每轮只跑一条
  低优先就 `break`（eng-34 让路）；当该条是**被拒收**请求（取消 / 超时 / 空分区
  / 准入失败——`write/gate.rs:269-280` 的 `reply_gated_reject`，**名额正确归还**
  但**未派发写任务**）时，回主循环：两通道空、无在飞读、`write_flight` 空 ⇒
  守卫不 `continue` ⇒ `wait_event` 无任何事件源可注册 ⇒ 停在 kevent 长眠，
  写门里还剩 ~476 条永不被检查 ⇒ 整个 shard（所有分区 / 读 / reopen / 指标）
  停摆。埋点实证：`[prb] REJECT ... err=Some(NotRunning)` 紧接
  `[prb] PARK#594 wf_empty=true ready=true ... ready_li=1 pending=476 running=0`。
- **否证清单**：线索 2（名额被占不前进）——停滞时在飞写 0、`running==0`；
  线索 3（错误路径漏放名额）——`gate.finish` 已在拒收路径正确执行；
  send / crossfire 侧——维持前轮否证（push/pop 平衡、registry 归零）。
- **修复**：park 判据补 `|| gate.has_takeable()`；新增
  `WriteGate::has_takeable()`（**不能**用 `has_ready()`：额度满时 `take_next`
  会 `push_front` 返回 `None` ⇒ `has_ready()` 恒真 ⇒ 无 await 忙转、饿死同线程
  在飞写任务，实测 `write_concurrency` 该用例 120s 超时）。另修**独立的测试
  前提 flake**：`low_pq_depth > 0` 断言由固定 `sleep(10ms)` 改为有界轮询
  （load ≥50 下 24/30 假红）。
- **验证**：`fkv_shard` 146/146；全量 **1505 passed / 1 skipped / 0 failed**；
  冻结快照 backpressure **8/8（~0.5s，0 守卫触发）**；load≈33 下 nextest 10/10；
  直接二进制 A/B（load≈70，30 对）**pristine HANG=6 vs fixed HANG=0**，
  fixed 复跑 25/25。
- **等待上界结论（修正原「建议处置 2」）**：`queue_timeout` 与 `request_timeout`
  对**本挂起**均无效（请求已在写门队列、deadline 检查点在出队之后；且 park 后
  整 shard 停摆，调用方超时无法恢复引擎 liveness）。有界默认属可选加固（C++
  侧无对应上界，语义会签见第 34 轮索引「语义/盘面格式」段），本轮不改默认。
- 取证工件（未提交）：`/tmp/r29/io32probe/`（修复 patch / pristine 用例原文 /
  埋点日志）、`/tmp/r29/io32-cfx`（crossfire 探针模板）。

## 复审记录（2026-11-08，第 36 轮异步域）

- 判定：**达标**，移入 reviewed/。逐行核对成立（park 判据补 `gate.has_takeable()`；
  `has_takeable` 不变量「就绪队非空 ⇒ 额度未满时 `take_next` 必可取」经
  push/take_next/finish/unstart/remove_pending/clear 全路径核对；「不能用
  `has_ready`」的反向理由复核成立）。
- 压力与判别力：`queue_backpressure` ×12 **0 守卫触发**（load 21-23）；去掉
  修复 → 6 连跑 1 次守卫失败（load 低，如实）；`has_takeable` 误用 `has_ready`
  → `write_concurrency::max_write_concurrency_one_serializes_partitions`
  **TIMEOUT 120s**（精确复现推理）。
- 同类缺陷系统扫描（park 判据漏「非空但无事件源」内部队列）：**零同型、零反例**
  （pending/wake/reopen/large_park/blocked_read/deferred/write_flight +
  prewarm 信号/crond/standby/S3 worker 逐点论证）。⇒ 本条为本次 liveness 面
  收口，无后续条目。
