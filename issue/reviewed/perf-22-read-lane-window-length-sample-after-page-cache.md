# perf-22 页缓存全命中后 bench 多链读相位每相位出现一个 ≈窗口时长的 get 样本（max = 窗口 − ~7.5ms）：读路径失去自然 IO 挂起点后的单任务调度公平性问题，根因未定位

- 领域：性能（bench 读链并发 × 异步调度交叉；建议异步域会签）
- 优先级：P3（bench lane 形态下才复现的计量面问题；吞吐 / avg / p50 /
  p99 均不受影响——单样本，对 avg 贡献 ~1µs；但它是 `6f79f44` 读路径
  页缓存**直接显性化**的行为变化，且机理在生产单线程 runtime 上同样
  存在理论风险，需 runtime 侧确认）
- 置信度：现象高（6/6 多链相位复现 + 单链阴性对照 + PRE 三相位阴性
  对照 + 3s/5s/2s 三种窗口窗口锁随动）；根因低-中（静态推演，未做
  runtime 级 trace）

## 现象（fkv `0b87909` vs `c5e8fbc`，同机 darwin arm64 12 核）

`--workload read-only --rows 20000 --partitions 2 --read-per-part 2
--test-secs N`（4 条并发读链，`futures::join_all` 单任务内驱动，
bench `read.rs:159-178`）在 `0b87909`（页缓存读路径）下，**每个读相位
恰有一个 get 样本 elapsed ≈ 窗口时长 − ~7.5ms**：

| 相位 | 窗口 | get_max_us | 备注 |
|---|---|---|---|
| REL read-only p1 | 5s | 4,992,802 | load ~9.2 |
| REL read-only p2 | 5s | 4,992,720 | load ~9.4 |
| REL read-only（复现） | 3s | 2,992,538 | 窗口锁随动 |
| REL write-read p1/p2 | 2s | 1,991,489 / 1,992,031 | 窗口锁随动 |

阴性对照：

- **单链**（`--partitions 1 --read-per-part 1 --test-secs 3`，同二进制）：
  get_max = **2,788µs**，无窗口长样本；
- **PRE（c5e8fbc，无读路径页缓存）多链** 5s/2s/3s 三相位：get_max =
  256 / 208 / 182µs，无窗口长样本；
- 历史各轮（第 17-31 轮 artifact）read lane max 从未出现窗口长量级。

sum 佐证恰为一个样本：REL read-only p1 avg=4.24µs × count≈4.65M ≈
19.7s 总时 = 常规样本 ~14.7s + 单个 4.99s 样本（4 链 × 5s 窗口 lane-秒
口径自洽）；若 ≥2 个窗口长样本 avg 会显著抬高。

## 机理推演（未证）

页缓存全命中后点读 `store.get` 的 await 链（`read_entry` →
`descend_step` → `CachedReadIo::read_page_owned` 命中分支
`page_cache.rs:559-573`）**不再有任何返回 Pending 的 IO 点**；async fn
对已 ready 的 await 不挂起，`join_all` 的单任务 poll 可在某条链上长距离
推进。read 相位仍有 ~64k 次 miss（≈每 77µs 一个挂起点）供链间轮转，但
某条链的在飞 get 从 ~7.5ms 处起直到窗口末才被再次 poll——与
`join_all` 单任务内「就绪饱和 + 唤醒不改变已在运行的任务」的调度边界
一致。bench 侧 `open_store_li`（`runner.rs:132-157`）未设
`with_yield_budget_us`（Store 默认 0：点读 `read_entry` 的
`YieldBudget`（`fkv_task_read/src/lib.rs:556,568`，逐 descend 步检查）
与 overflow 链 `read_overflow_budget`（`lib.rs:177-195`）在预算 0 时
全部直通）。确切机制需要 runtime 级
trace（哪条链、挂起在哪个 await）才能定论。

## 风险面

- bench：`get_p999/p9999/max` 报出窗口长量级（叠加 perf-21 的分位缺陷
  显性化）；吞吐 / p50 / p99 / avg 无影响。
- 生产：单线程 runtime 上「热页缓存读循环移除自然 IO 让出点」的形态与
  本观测同源；生产路径由 runtime 注入 `yield_budget_us`（SDK 默认 20µs）
  且点读循环短，风险理论化，但值得异步域对照 C++ buffer pool 命中路径
  （C++ 同步代码无此问题）确认一次调度边界。

## 建议定位路径

1. bench `read.rs` 每链记录「最后完成样本的 lane id + 挂起区间」复跑；
2. 或临时给 bench 注入 `with_yield_budget_us(20)` 复测：若窗口长样本
   消失则坐实让出点缺失机理，修法即 bench 默认开启预算或 read lane 定
   期 `yield_now`。

（发现轮：第 32 轮性能域，验证 `6f79f44` 时显性化；PRE 阴性 + 单链阴性
已排除「负载噪声」与「页缓存正确性」两个解释。）

## 实现记录（2026-11-07，根因定位 + 修复 fkv commit `1a48b8c`）

- **建议路径 2（yield 预算注入）先行证伪**：`/tmp/r29/fkv-rel` 实验
  `with_yield_budget_us(20)` 复跑，窗口长样本仍在（max 4,992,694µs）——
  让出预算缺失不是根因。
- **临时探针定案**（lane id + 完成数 + 长样本签发时刻，跑后删除）：读相位
  4 链中 lane0 **独占整窗完成 4,561,614 次 get**，其余三链 5s 内仅完成
  3,286 / 1,345 / 838 次；三条链各自最后一个在飞 get 签发于 ~7.6-8.1ms、
  完成于窗口末（4.99s）——即观测到的窗口长样本是**被饿死链路的在飞 get
  等待重 poll**，不是引擎慢。机理：页缓存全命中后 get 全链路不再返回
  Pending，`futures::join_all` 单任务内就绪饱和的链路可以整窗不让出；
  工作集热身完成后（~64.5k 次 miss 集中在前几 ms）唯一自然让出点消失。
- **修复**：`fkv_bench/src/read.rs` 读链每 `YIELD_EVERY = 64` 次 get
  协作让出一次（本地 `yield_now`，同 `fkv_task_read::yield_now` 的自唤醒
  语义，不为此引入 engine 依赖）。实测：get_max 110µs→373µs 量级（量子
  ~64×单次 get×链数），p99 229µs / avg 4.66µs，吞吐 833k-901k kv/s
  （公平 4 链口径，vs 饿死形态的 936k 单链吞吐 -4~11%）。逐 get 让出的
  对照实测 217k（执行器往返税 ×4）故取 64 粒度。
- 生产面结论：生产请求走 shard 事件循环逐请求完成（队列 FIFO），不存在
  join_all 多循环同任务的形态；该缺陷为 bench 计量面（多链「并发读者」
  口径此前实为单链吞吐），引擎无缺陷。异步域第 32 轮对 `6f79f44` 的
  审查结论不受影响。
- 变异 / 复验：修复后 3s/5s 窗口 × 多次复跑 max 恒在亚毫秒；fkv_bench
  套件 58/58；fmt/clippy 绿。

## 复审记录（2026-11-07，第 33 轮性能域 + 异步域双复核）

- 判定：**达标**，移入 reviewed/。实现复核：本地 yield_now 语义注释 /
  YIELD_EVERY=64 常量化 / 分层正确（不为 bench 引入 engine 依赖）；实测
  5 个多链相位 max=572µs-12.3ms 全部脱离饿死形态（比窗口低 3 个量级），
  p99 229-236µs、吞吐 770-822k（记录 833-901k 的高底噪偏移 -2~-8%）；
  单链对照 max 113µs 吻合。异步域会签：生产读走每请求一 spawn +
  有限批 join_all（全仓枚举），无「join_all 包无限循环链」形态，引擎
  无缺陷结论成立。
