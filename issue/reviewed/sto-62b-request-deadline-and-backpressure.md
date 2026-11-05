# sto-62b 请求级 deadline 与队列背压

## 来源

`issue/new/sto-62-*` 的「无超时」风险。取消令牌已在 `sto-62a` 落地，本条补
**服务端 deadline** 与**入队背压**。

## 位置

- `crate/engine/fkv_shard/src/lib.rs`：`ShardSender::request` / `enqueue`、
  `process_envelope`、`run_read_batch`、主循环 `rx.recv()`。
- `crate/engine/fkv_runtime/src/lib.rs`：11 处 `sender.request(...)` 调用点。
- `crate/engine/fkv_shard/src/backend.rs`：Cloud 后端慢链路（S3 超时已映射
  `KvError::Timeout`，但只到单个 IO，不覆盖整请求）。
- `crate/foundation/fkv_config/src/lib.rs`：无请求超时配置项。

## 缺口

1. 无 per-request deadline：`Envelope` 不带截止时刻，shard 无法在开始执行前
   快速拒绝已过期请求；云模式慢链路会让请求长期占用 shard 队列位。
2. `queue_size` 满时 `send().await` 无界等待，调用方无法区分「背压」与「挂死」。
3. `fkv_config` 没有请求超时字段，runtime 只能靠调用方自行 `timeout`。

## 方案

1. `Envelope` 增 `deadline: Option<Instant>`；`process_envelope` 在取消检查之后、
   `admit` 之前比较 `Instant::now()`，过期直接回 `KvError::Timeout`（不执行）。
2. `ShardSender` 增 `request_deadline(req, Duration)`：deadline 同时写入 `Envelope`
   并作为本端 `RequestHandle` 的兜底；本端超时仍走 `sto-62a` 的取消标志。
3. 入队背压：`queue_size` 满时改用「有界等待 + `KvError::Busy`」，或在
   `KvOption` 增 `request_queue_timeout_us`（0 = 无限等待，保持现状默认）。
4. 配置项落地后，`Runtime` 的便捷方法统一带上默认 deadline，云模式默认值应覆盖
   一次 S3 重试窗口。

## 验收

- 注入慢 IO（现有 `fkv_failpoint` 或 Mem 后端延迟）后，过期请求回
  `KvError::Timeout`，且该请求不进入 `admit` / `prepare_table`。
- 队列打满时新请求在配置时长内返回 `Busy`，不是永久挂起。
- 默认（配置缺省）行为与当前一致，现有测试全绿。
- `cargo clippy ... -D warnings` 零警告。

## 重叠

- 取消标志与回执契约：`issue/done/sto-62a-oneshot-cancel-token.md`。
- 云后端单次 IO 超时映射：`crate/io/fkv_object_s3/src/lib.rs`（已具备，不改）。
- 请求类型覆盖：`issue/todo/12a-*` / `12c-*`。

## DONE（2026-11-05）

### 实现

- `crate/engine/fkv_shard/src/lib.rs`
  - `Envelope` 增 `deadline: Option<Instant>`，`expired()` 只在开始执行前判断。
  - 三条开始执行路径统一过期拒绝：`process_envelope`（取消检查之后、`admit` 之前）、
    `run_read_batch`（批内逐条）、`run_put_group`（group commit 前）。过期请求回
    `KvError::Timeout`，不进入 `admit` / `prepare_table` / store，并记
    `record_request` + warn 日志。
  - `ShardSender` 增 `request_deadline(req, Duration)`：deadline 同时写入 `Envelope`
    并由本端 `futures_timer::Delay` 兜底，超时回 `Timeout` 且 drop 句柄置取消标志
    （复用 sto-62a 语义；已开始请求不中断）。
  - `request_at(req, Instant)`：只带服务端 deadline、不做本端兜底。
  - 入队背压：`with_queue_timeout` 配置有界等待；队列满时取「请求 deadline 与
    queue_timeout 的较早者」，分别回 `Timeout` / `Busy`，warn 日志带 shard/op/wait。
    未配置时保持原无限等待（默认行为不变）。
- `crate/engine/fkv_runtime/src/lib.rs`
  - `Runtime::with_request_timeout` / `with_queue_timeout`：数据面默认值，经
    `Runtime::request` 的全部便捷方法与 `fanout` 生效；`attach_shard` /
    `detach_shard` / `shutdown` 不套用（控制面必须确定性回执）。
  - 新增 `Runtime::request_deadline` / `request_at` 显式覆盖入口。

### 测试

- `crate/engine/fkv_shard/tests/deadline.rs`：过期 Put 回 Timeout 且不落盘、读批混批
  过期/有效请求、`request_deadline(Duration::ZERO)` 超时且不落盘、未过期 deadline
  正常执行。
- `crate/engine/fkv_shard/src/lib.rs` 单测：队列满按 queue_timeout 返回 Busy；
  未配置时等槽位释放后投递成功；队列满 + 过期 deadline 优先回 Timeout。
- `crate/engine/fkv_runtime/tests/deadline.rs`：Runtime 默认 deadline 生效且请求未落盘、
  显式 `request_deadline` 覆盖运行时默认。
- 通过：`cargo test -p fkv_shard --lib --test deadline --test cancel --test backend
  --test read_batch --test write_gate --test priority`；
  `cargo test -p fkv_runtime --test deadline --test runtime --test write_group_commit`。
- `cargo clippy -p fkv_shard -p fkv_runtime --tests --no-deps -- -D warnings` 零警告；
  带 workspace 依赖的 clippy 会命中 fkv_cloud 并发改动中的 `collapsible_if`，非本条范围。
- `cargo +nightly fmt -p fkv_shard -p fkv_runtime`。

### 边界与遗留

- 按本次改动范围未动 `fkv_config`；`KvOption` 的请求超时 / 入队超时键留给后续条目，
  当前通过 `Runtime` / `ShardSender` builder 配置。
- 「云模式默认 deadline 覆盖一次 S3 重试窗口」未自动启用：默认值保持 `None`，避免改变
  现有云模式行为；需要时由调用方显式设置。
- 与 eng-32（work loop 并发化）不冲突：deadline 检查绑定在 admission 边界，eng-32 把
  请求 spawn 成独立 future 后，每个请求仍须在开始执行前保留该检查；写 gate 排队请求
  在 `take_next` 后进入 `process_envelope`，同样受检。
- 风险：本地兜底依赖 `futures-timer`（仅配置 timeout 时创建 `Delay`，默认零开销）；
  请求已开始后 deadline 不中断，调用方可能收到 `Timeout` 而写已生效（与 sto-62a / C++
  请求不可中途取消一致）。

## 审查结论（dragon 独立复核，2026-11-04）

**核心已实现且被测试覆盖；剩一条「生产不可达」。** 逐条核对：

- `Envelope.deadline` + `expired()`：`fkv_shard/src/lib.rs` 定义处，并在三处执行路径
  生效 —— `process_envelope`（取消检查之后、`admit`/`prepare_table` 之前回
  `Timeout` 且 `record_request` 记一笔）、以及 `run_read_batch` 的过期分支
  （读批内过期与存活请求分流）。✔ 对齐本条方案第 1 点。
- `ShardSender`：`request_with_timeout` / `request_deadline` / `request_at` +
  `with_request_timeout`（本端兜底：超时 drop 句柄 → `RequestHandle::drop` 置取消标志，
  执行中不打断）；`with_queue_timeout` + `queue_wait` 取「入队 timeout 与请求 deadline
  较早者」并分别回 `Busy` / `Timeout`。✔ 方案第 2、3 点。
- 测试：`tests/deadline.rs`（过期 Put 不落盘、读批过期/存活分流、`request_deadline`
  跳过 store、存活 deadline 正常执行）+ `src/lib.rs` 内的 `enqueue_full_*` 单测
  （满队列 → 按时 `Busy`；未配 timeout → 等槽位；满队列 + 已过期 → `Timeout`）。
  ✔ 验收第 1、3 条。

**未做（本条剩余的唯一缺口）**：`queue_timeout` / 请求 deadline **在生产不可达** ——
全仓没有 `with_queue_timeout` / `request_deadline` 的调用点，`fkv_config::KvOption`
也没有 `request_queue_timeout_us`（`max_processing_time_us` 有完整
`KvOption → fkv_runtime → ShardConfig` 链路可照抄）。因此「队列打满时新请求在配置
时长内返回 `Busy`」目前只有单测证据，没有生产开关；`Runtime` 便捷方法的默认
deadline（云模式要覆盖一次 S3 重试窗口）也还没定。

**一条实测教训（给后续补集成测试的人省一次踩坑）**：我用真实 shard 填满通道来写
「满队列 → `Busy`」的集成测试，两次都失败 —— ① 用低优先 `Gc` 造积压：空表上单条
`Gc` 只要几微秒，几百条积压会在测试发出背压请求之前就排空；② 换成 4096 条高优先
`Put` 积压：通道（`queue_size = 1`）在写批落下期间会被快速抽干，投递常常刚好拿到
槽位。结论：**背压应该在 sender 层用「预填满的假通道」测（就是现有 `full_sender`
单测的做法）**，真实 shard 上的积分测试只在配置项落地后再补，且要按
`write_queue_depth > 0` 之类信号轮询，不要靠固定 sleep。

## 复审（2026-11-05 r6）

**通过（核心实现 + 测试；「生产不可达」缺口维持 dragon 复核结论，留待配置项接线）。** 核实：

- `fkv_shard/src/lib.rs:165` `Envelope.deadline`、:177-178 `expired()`；:302-312 `request_deadline`/`request_at`；:322 `with_queue_timeout`；:414-421 `queue_wait` 取 deadline 与 queue_timeout 较早者并区分 `Timeout`/`Busy`（:420-421）；:1449-1453 过期请求在 admit/prepare_table 之前回 `Timeout` 并 warn。
- `fkv_runtime/src/lib.rs:1271/1279` `with_request_timeout`/`with_queue_timeout`、`request_deadline` 显式覆盖。
- 测试 `fkv_shard/tests/deadline.rs:100/129/153/175` 四条（过期 Put 不落盘 :110-118、读批分流 :141、request_deadline 跳 store :163、存活正常执行）+ `fkv_runtime/tests/deadline.rs:71/106` 两条；单测 `enqueue_full_*`（queue_wait/Busy）在 lib 内。
- 残余（KvOptions 无 `request_queue_timeout_us` 键、生产默认 None）已在 dragon 复核段如实记录，属已知边界而非假完成；如需收口应另立小条目挂 fkv_config。
