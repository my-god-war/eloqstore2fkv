# gap-04 V5 崩溃恢复缺「随机 kill point fuzz 闭环」（P2，本机可做）

## 来源

dragon 的剩余工作盘点；`doc/migration.md` §5 自述 V5 现状。

## 证据（2026-11-05 实测）

- `doc/migration.md` §5：V5 现状 = 「`fkv_failpoint` + `fkv_io_sim::crash/restart` 的
  **定向回归**；尚无 24h / 随机 kill point 的 crash fuzz 闭环」。
- 设施其实都已具备：`fkv_failpoint::fail_point!`（如
  `fkv_io_compio/src/lib.rs:2201` 的 `SyncDir`）、`fkv_io_sim` 的
  `crash()` / `restart()`（`fkv_io_sim/src/lib.rs:388-396`）、以及
  `fkv_ledger fixture` 的 `status=fail` 语义。
- 缺的是**闭环**：没有「随机注入 N 个 kill point × 每点重启后校验不变量」的驱动器。

## 问题

定向回归只能覆盖「我们已经想到的」崩溃点。真实崩溃（撕裂写、manifest 半条记录、
目录项未 fsync）集中在**没想到**的位置。本项目历史上已有两个此类真实缺陷
（`sto-51` 撕裂尾、`sto-52` GetMapper 重建），且 `COMPAT_BASELINE` 把
「恢复语义不等价」列为**不纳入互操作保证**——这条排除本身就是「没有闭环证据」的结果，
一旦有了闭环，排除范围可以收窄。

## 目标

1. 写一个**确定性**的崩溃注入驱动器（不依赖时序）：
   - 输入：一条 op log（见 `gap-03`）+ 一个 kill point 序号；
   - 动作：跑到第 k 个 `fail_point` 就中止并 `crash()`，然后 `restart()` 重放；
   - 校验不变量：`manifest` 可重放、已确认的写不丢、未确认的写不产生可见半值、
     GC 不删被引用文件、目录/文件 fsync 次序不产生悬挂引用。
2. **随机枚举**：对每条 op log 遍历全部 kill point（数量可控时穷举；不可控时按种子
   抽样），记录失败点并可复现（种子 + k 一起打印）。
3. 夜间 CI 跑一个加大预算的版本（本机放小预算的 smoke 版本）。

## 验收

- 存在 `cargo nextest` 用例：给定 op log + kill point 列表，全部通过不变量校验；
- 人为制造一个已知崩溃缺陷（例如先 `remove` 段文件再 `sync_data`，参考 `sto-51`）时
  该用例必须失败（反向验证）；
- 失败输出包含 `seed` / `kill_point` / `op_index`，可一键复现；
- `doc/migration.md` §5 的 V5 描述更新为「有闭环 + 覆盖点数」。

## 阻塞 / 依赖

**无外部阻塞**：本机可做（`fkv_io_sim` + `fkv_failpoint` 都已就位）。
需要的只是「不变量清单」的评审（建议按 C++ `Replay` / `GetMapper` 语义逐条列，
而不是凭印象）。

## 核验记录

- 实测：`doc/migration.md` §5 V5 段；`fkv_io_sim:388-396`；
  `fkv_io_compio:2201` 的 `SyncDir` failpoint。

## 复审（2026-11-05）

原 index 把它归为“验证债务”不成立：`fkv_io_sim` 没有任何 crate 依赖，
`kill_point` 的 abort 也从未被触发过。先做 gap-07 的接线，再做闭环。
