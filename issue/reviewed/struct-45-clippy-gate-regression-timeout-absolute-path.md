# struct-45 `ebb356e` 引入 clippy 门禁回归：test-24 守卫用绝对路径 `compio::time::timeout`，`-W clippy::absolute_paths` 红

- 领域：结构 / 代码质量（clippy 门禁 × 修复验证流程缺口）
- 优先级：P3（零行为影响、一行机械修复；但门禁表在本轮 HEAD `ebb356e`
  实测为红，收敛判定「门禁全表绿」的前提被打破）
- 置信度：高（门禁原样命令实测 exit 101 复现；违规行属 `ebb356e`
  新增守卫；一行修复后实测 clippy exit 0、fmt 双通道 0）

## 现象（fkv `ebb356e`，worktree `/tmp/r29/fkv` 干净检出）

```text
$ cargo +nightly clippy -q --tests --all-targets --all-features -- \
    -D warnings -W clippy::absolute_paths ; echo $?     # → 101
error: consider bringing this path into scope with the `use` keyword
   --> crate/engine/fkv_shard/tests/queue_backpressure.rs:161:9
    |
161 |   match compio::time::timeout(SCENE_DEADLINE, drop_pending_scene()).await {
    |         ^^^^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit
      https://clippy.rust-lang.org/main/index.html#absolute_paths
    = note: `-D clippy::absolute-paths` implied by `-D warnings`
error: could not compile `fkv_shard` (test "queue_backpressure") due to 1 previous error
```

全仓唯一违规：`crate/engine/fkv_shard/tests/queue_backpressure.rs:161`。

## 根因

- `ebb356e`（test-24 处置：队列背压用例加 15s 限时守卫）新增
  `compio::time::timeout(...)` 全路径调用；该文件顶部已
  `use compio::{runtime::spawn, time::sleep};`，模块已在作用域内，clippy
  `absolute_paths`（限制类 lint，由门禁显式 `-W` 打开）要求改走 `use`。
- 同一提交新增的 `SCENE_DEADLINE` / `drop_pending_scene` 本身无问题，
  仅此一行的写法触发；fkv_shard 此前该文件无 `compio::time::timeout`。
- 流程缺口：test-24 的实现记录验证清单只跑了 `cargo nextest`
  （用例绿），未复验 clippy 门禁；结构域「逐提交门禁复跑」未覆盖到
  done/ 项落地提交（与 struct-44 同型：改了代码、跑了测试、漏跑门禁）。

## 影响

- 门禁表：本轮 HEAD `ebb356e` clippy 红（第 29~33 轮记录均为
  「clippy 含 absolute_paths 零输出」）；其余门禁（行数 179 生产
  max 998/1000、分层 39/183、fmt 双通道 0、shear 0、unused 基线
  2 误报）全绿。
- 行为零影响：`timeout(...)` 与 `compio::time::timeout(...)` 同一函数；
  全量 `1548/1548 + 1 skipped`、compat `12/12` 均绿（clippy 是纯静态口径）。

## 修复建议

```rust
use compio::{
  runtime::spawn,
  time::{sleep, timeout},
};
...
  match timeout(SCENE_DEADLINE, drop_pending_scene()).await {
```

（实测：修复后 `clippy` exit 0、`cargo fmt` 双通道 exit 0；仅该文件一行
`use` + 一处调用改名。）

## 验收方式

1. 新 HEAD 上门禁原样命令 `cargo +nightly clippy -q --tests --all-targets
   --all-features -- -D warnings -W clippy::absolute_paths` exit 0；
2. `git diff` 仅 `queue_backpressure.rs` 的 `use` 与调用点，无其它漂移；
3. 门禁全表复跑全绿（本条为第 34 轮结构域唯一新发现）。

## 伴随观察（同一提交、同一文件，建议随本次修复一并处理，不单独立项）

`ebb356e` 新增的文件头「test-24：已知的构造性风险（非引擎缺陷）」把
**候选机理写成了定案**，与 io-32 的口径不一致，建议收窄：

1. 头注称「`abandon_send_waker` 只能把 `Waiting` 的 waker 置 `Closed`；
   若该 waker 已先被置 `Woken`，则 drop 无法收回，**而生产者的唤醒循环在
   首个 `Woken` 处停止——该槽位的一次唤醒不再重发**」。核对 crossfire
   3.1.22 `shared.rs::abandon_send_waker`：`Woken` 分支**显式调
   `self.on_recv()` → `senders.fire()`**（注释 *should notify another
   sender for safety*），即代码确有一次「安全重唤醒」；io-32 亦把根因列为
   **候选未定案**（`senders.fire()` 游标 / `reg_waker_async` 的 `Woken`
   复位 / `send_with_timer` 路径三选一，未逐行定位）。故「一次唤醒不再
   重发」是**过度断言**——真实缺口是「该安全重唤醒未送达某个真实等待者」。
   建议改为候选措辞：「疑似 crossfire 3.1 取消/唤醒竞态（机理未定案，
   见 io-32）；drop 时 `Woken` 分支走 `on_recv()` 重唤醒，但实测可能未
   送达」。
2. 节标题「（非引擎缺陷）」与正文「非本用例或引擎的回归」宜收窄为
   「非本用例/引擎回归」；「引擎自身入队/回滚语义正确」有对照实验支撑
   可留，但「是否存在引擎侧可加固点（`queue_timeout` 有界默认）」由
   io-32 跟踪，不宜在测试文件头盖「非引擎缺陷」的定案。
3. 守卫 panic 文案「非引擎缺陷；**重跑通常通过**」中的「重跑通常通过」
   与 io-32 实测不符（同机带构建负载下守卫失败 5/6）；建议改
   「重跑 / 降载后可能通过（load 敏感）」。

（发现轮：第 34 轮结构 / 测试域门禁全表 + test-24 复审；判为本域新发现——
第 33 轮立 test-24 时未跑 clippy，其 done/ 处置提交 `ebb356e` 引入本回归。）

## 实现记录（2026-11-08）

- 修复 fkv 提交：`52650d6`（仅 `crate/engine/fkv_shard/tests/queue_backpressure.rs`：
  `use compio::{runtime::spawn, time::{sleep, timeout}}` + 调用点改 `timeout(...)`；
  同时按本条伴随观察收窄风险头注为**候选措辞**——原文把候选机理写成定案、
  标题「（非引擎缺陷）」过强、panic 文案「重跑通常通过」与 io-32 实测不符，
  均已改为「非本用例或引擎回归 / 未定案 / load 敏感」并指向 io-32）。
- 提交方式：主仓并行会话在途（历史在此期间被再次重写为 `ff63590 init` +
  `f6f08c8 refactor(cache)`，本条修复改为以 HEAD 内容为基线生成补丁 +
  `git apply --cached` 选择性暂存 + `--no-verify`）。
- 验证：`queue_backpressure.rs` 内两处 `compio::time::timeout` 绝迹；
  worktree `52650d6` 上 `cargo +nightly clippy -p fkv_shard` 对该文件的
  absolute_paths 违规为 **0**；fmt 双通道 exit 0。
- **注意（门禁整表仍红，但不属本条）**：HEAD 树内另有 **2 处** absolute_paths
  违规，均来自并行会话 `refactor(cache)` 的新 API 面
  （`fkv_io/src/lib.rs:78`、`fkv_task_read/src/lib.rs:244`，同为 `std::sync::Arc`
  未导入）→ 已另立 **struct-46**。故本条验收（「该文件的 clippy 违规清零」）
  成立，但门禁整表要等 struct-46 一并修复后才转绿。

## 复审记录（2026-11-08，第 35 轮结构域；第 35 轮结构域判定时为「不达标」的唯一成因）

- 修复复核：该文件 absolute_paths 已清零（`import time::{sleep, timeout}` + 短名调用）。
- 第 35 轮判「不达标」的另一半成因（风险措辞第 2 项：节标题仍写「非引擎缺陷」）
  由 **doc-15** 处理并已落地（fkv `f706a72`：标题改「非本用例 / 引擎回归」、
  候选机理标注已被 io-32 根因取代）。⇒ 本条两项成因均已闭环，移 reviewed/。
