# r8d-01 P3：write_gate.rs 模块文档仍写「work loop 串行、跨分区写不并行」，与已落地的 eng-33a 相反

## 现象

`crate/engine/fkv_shard/src/write_gate.rs:8-10`（2026-11-05 05:2x 亲读，该文件
mtime 02:44，本轮审查期间未被并行会话触碰）：

```rust
//! 当前 work loop 仍是串行执行（eng-32），因此跨分区写不会真正并行，本结构的
//! 收益只在「同一分区写严格 FIFO + 全局额度可计数」；写路径并发化后（eng-33a）
//! 换掉驱动方即可，队列语义不变。
```

同 crate 已落地的并发派发事实（内容锚点亲读，行号为 05:19 版本）：

- `crate/engine/fkv_shard/src/lib.rs:1936-1947` 附近：`drain_write_gate` 中高优先
  Put 组经 `state.stores.take(&tbl)` 取走独占 Store 后
  `write_flight.push(spawn_put_group(...))` 派发独立 compio task；
- `WriteFlight` / `WriteDone` 结构与主循环顶部 `write_flight.try_next()` 回收
  （lib.rs 主循环「回收已完成的写任务」段）；
- `crate/engine/fkv_shard/src/partition.rs` 模块文档首行即写「每分区独立 Store：
  跨分区写可并发，同分区仍由 WriteGate 串行」；
- 验收测试 `crate/engine/fkv_shard/tests/write_concurrency.rs` 头注释明言
  「两个分区被同时派发时采样必然能观测到 2」。

即：write_gate.rs 自述的「当前仍是串行 / 并发化后换驱动方」前置条件已由 eng-33a
兑现并复审归档（`issue/reviewed/eng-33a-write-gate-concurrent-dispatch.md`），
模块头三行停留在落地前状态，与同目录 partition.rs、同 crate 主循环及测试的
陈述直接矛盾。

## C++ 对照

无差异——本条纯为 Rust 侧文档滞后，不涉盘面 / 语义。C++ 侧对应机制
`PendingWriteQueue` + `max_write_concurrency_` 的行为与 WriteGate 现实现一致。

## 目标

- 把 write_gate.rs:8-10 改写为落地后口径：驱动方已是 eng-33a 的并发派发
  （跨分区真并发、同分区 FIFO、全局 `max_write_concurrency` 额度），
  与 `partition.rs` / `02-execution-model.md` 的表述同源；
- 不改任何实现代码与队列语义。

## 验收

- `grep -n "仍是串行\|不会真正并行\|写路径并发化后" crate/engine/fkv_shard/src/write_gate.rs`
  零命中；模块头与 `issue/reviewed/eng-33a-*` 的落地描述一致；
- `./sh/clippy.sh` 与 `./test.sh` 通过（纯注释改动，预期无行为影响）。

## 核验记录

- 去重：rev-48（`issue/done/rev-48-write-concurrency-doc-lag-after-eng-33a.md`）
  验收范围显式限定 `doc/`（其开发计划第 3 条 grep 只扫 `doc/`），in-code 模块
  注释不在其清单；rt-02 范围仅 Gc 映射文档；`issue/new/`、`issue/todo/` 无其它
  条目承接 in-code 写门文档。
- r8d-01（2026-11-05）：R4 独立审查亲读 write_gate.rs（02:44 版，未被并行
  会话改动）与 lib.rs / partition.rs / write_concurrency.rs 后立项。
- 并行写入区提示：fkv_shard/src/lib.rs（05:19）、fkv_sdk/src+tests（05:19-05:20）、
  fkv_cli/src+tests（05:18）在本轮审查末段仍有 <2 分钟 mtime 的改动（KvStore::new
  增加 term 参数一批），本条锚点文件 write_gate.rs 不在其列，行号以内容锚点为准。


## DONE（2026-11-05，第二轮复审修复）

DONE：`write_gate.rs` 模块文档改为已按 `WriteFlight` 跨分区并发派发。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 亲读 `crate/engine/fkv_shard/src/write_gate.rs`（mtime
05:28，非写入区）：

- 模块头 :8-10 已改为「高优先 Put 组由 `WriteFlight` 按分区派发成独立 task，跨分区
  真正并行；`running` / `max_concurrency` 同时承担并发额度与背压计数（eng-33a）。同一
  分区仍严格 FIFO，队列语义不因并行化改变」，与 `partition.rs` / `02-execution-model.md`
  同源。
- 验收 grep `仍是串行\|不会真正并行\|写路径并发化后` 于该文件零命中（exit=1）。
- 纯注释改动，未动队列语义；实现侧 `spawn_put_group` / `WriteFlight` 断言在
  eng-32c / eng-33a 条目已核。
