# sto-61a Shard 生命周期状态机：attach / detach 幂等与非法转换

## 来源

`issue/new/sto-61-store-lifecycle-no-state-machine-no-multi-shard-hosting-attach-detach-divergence.md`
的状态机与 attach/detach 语义部分（多 shard 托管见 sto-61b，Local term 见 sto-61c）。

## 位置

- `crate/engine/fkv_shard/src/lib.rs`
  - `ShardState`：`attached: bool` + `expired: bool` → `lifecycle: DsShardState`
  - `DsShardState` / `AttachKind` / `ShardState::admit_attach` / `ShardState::admit_detach`
  - `admit`、`dispatch` 的 `AttachShard` / `DetachShard` / `Put` / `PutLarge` 臂
- `crate/engine/fkv_shard/tests/lifecycle.rs`（新增）
- `crate/engine/fkv_runtime/tests/runtime.rs`（换 branch 需先 detach 的断言）

## 落地内容

1. 用 `enum DsShardState { Detached, Attached, Expired }` 替代两个互斥 bool，
   普通请求准入改为单次 `match`：`Detached → NotRunning`、`Expired → ExpiredTerm`、
   `Attached → 放行`（对齐 C++ `AdmitRequest`）。
2. `admit_attach` 集中判定转换：
   - `Detached`：允许，重建 Store 状态；
   - `Attached` + 同 shard / branch / term：幂等，**不** `prepare_attach`、不清缓存；
   - `Attached` + 旧 term：置 `Expired` 并返回 `ExpiredTerm`；
   - `Attached` + 更高 term 或换 branch / shard：`InvalidArgs`（必须先 detach，
     对齐 C++ `AttachShard` 要求 slot 处于 `Detached`）；
   - `Expired` + 同 branch 且 term 不低于当前：恢复；旧 term：`ExpiredTerm`。
3. `admit_detach`：`Detached` 幂等成功；`shard_id` 不匹配返回 `InvalidArgs`；
   其余转换驱逐已打开 table 与 fd/路径缓存后置 `Detached`。
4. `Put` / `PutLarge` 的 term 不匹配仍把整 shard 置 `Expired`（行为不变，改走枚举）。

## 语义边界

- C++ 的 `Attaching` / `Detaching` 用于跨线程 CAS 与中间态拒绝请求。本实现控制请求
  由 shard 线程串行处理，中间态不跨请求可见，因此不落成状态；C++ 的
  `MarkShardExpired` 对应 `Expired`。
- `Detached` 后保留 branch / term：同一 branch 以旧 term 重新 attach 由 term 文件
  CAS 拒绝；这是 Rust 现有 fencing 契约，运行时有测试依赖（见 sto-61c 讨论 term 归零）。
- `Runtime` 无 Status 原子机：`Runtime::shutdown(self)` 消费所有权，API 上不存在
  Starting / Stopping 窗口；多 shard 生命周期留给 sto-61b。

## 验收

- [x] `cargo check -p fkv_shard -p fkv_store --tests` 通过。
- [x] `cargo nextest run -p fkv_shard -p fkv_store -p fkv_runtime --no-fail-fast`：
      161 passed, 1 skipped。
- [x] 新增 `fkv_shard::lifecycle attach_is_idempotent_and_keeps_state`：重复 attach
      不丢已写数据，重复 detach 幂等，detach 后普通请求 `NotRunning`。
- [x] 新增 `fkv_shard::lifecycle illegal_transitions_are_rejected`：换 branch /
      换 shard / 提 term / detach 错 shard_id 均 `InvalidArgs`；旧 term attach 后整
      shard `ExpiredTerm`，同 term 重新 attach 恢复。
- [x] `cargo clippy -p fkv_shard -p fkv_store -p fkv_runtime --no-deps
      --all-features --all-targets -- -D warnings` 零警告。

## 遗留

- 多 ds_shard 托管、per-request ds_shard、`partition_filter`、detach fanout：
  `issue/todo/sto-61b-multi-ds-shard-hosting-and-request-routing.md`。
- Local 模式 term 文件与 detach term 归零：`issue/todo/sto-61c-local-mode-term-file-and-detach-term-reset.md`。

## DONE（2026-11-04）

## 复审（2026-11-05 r6）

**通过。** 核实：`fkv_shard/src/lib.rs:1183` `enum DsShardState{Detached,Attached,Expired}`、:1206 `admit_attach`（同 owner 幂等 / 旧 term 置 Expired / 换 branch/shard/提 term `InvalidArgs`）、:1231 `admit_detach`（Detached 幂等 / shard_id 不符 `InvalidArgs`）、:1376-1378 准入单次 match（Detached→NotRunning、Expired→ExpiredTerm）。测试 `tests/lifecycle.rs:100` `attach_is_idempotent_and_keeps_state`、:142 `illegal_transitions_are_rejected` 在位。eng-32b 的读 spawn 改造保留了该状态机（控制臂仍串行），无回归。
