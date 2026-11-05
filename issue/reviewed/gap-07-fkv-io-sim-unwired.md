# gap-07 `fkv_io_sim` 未接线，崩溃/确定性重放无法在引擎级运行（P1）

## 证据（2026-11-05）

- `grep -rln fkv_io_sim --include=*.toml` 只命中 `crate/io/fkv_io_sim/Cargo.toml`。
- `fkv_store` / `fkv_runtime` / `fkv_testkit` 都没有 dev-dependency。
- `fkv_testkit/src/lib.rs` 自述 “crash harness 按需在此扩展”。
- `fkv_io_sim::crash()` 只是裸调用，不能按 kill point / op index 中止。

## 目标

1. 把 `fkv_io_sim` 接进 `fkv_store` / `fkv_runtime` 的测试依赖，至少能构造
   `Store<SimStore>` 并跑 open / put / get / reopen。
2. 提供可复现的崩溃点：按 op index 或 failpoint 名称在引擎路径中止，然后
   `restart()` 重放。
3. 不允许测试因为 “SimStore 不可用” 静默跳过。

## 验收

- `cargo nextest` 中存在 `Store<SimStore>` 的引擎级用例；
- 同一 op log 两次运行 snapshot 逐字节一致；
- 崩点失败输出 seed / kill_point / op_index。

## 复审归档（2026-11-05）

工作已落地，本轮复审逐项复核成立后归档到 `issue/reviewed/`。跳过 `done/`：这些条目的
实现在本轮复审之前就已提交，`done/` 只用于本轮亲自开发并附测试证据的条目。
`new/` 只留仍未落地的条目。

证据（复核命令与结果）：

- `crate/io/fkv_io_sim/src/oplog.rs`（行式文本 op log + `EngineOp`）、
  `crate/io/fkv_io_sim/tests/op_log_replay.rs`（6 条场景 × 每个 op 索引 crash→reopen→
  与干净前缀逐字节比对 + 子进程真 `!kill`）。
- `crate/engine/fkv_store/tests/sim_store.rs`：`fkv_io_sim` 已作为 dev-dependency 接入，
  `Store<SimStore>` 可 open / put / get / reopen。
- 本轮另在真后端补了 intra-op 闭环（见 `reviewed/gap-10`）。

无残余项。
