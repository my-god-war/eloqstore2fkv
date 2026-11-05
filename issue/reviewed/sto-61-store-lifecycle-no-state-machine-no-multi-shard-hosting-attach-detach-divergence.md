# sto-61 Store 生命周期：无 Status/DsShard 状态机、无多 shard 托管模型；Local 模式也写 term 文件；Attach/Detach 语义分叉

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:104-141`（`Store<I>` 纯数据结构，无状态机/无 mode 字段）、`crate/engine/fkv_shard/src/lib.rs:583-625`（attach/detach 处理）、`crate/io/fkv_io_compio/src/term.rs:1`（本地 `CURRENT_TERM_<branch>_<shard_id>` 文件）
- C++: `include/eloq_store.h:943-958`（`Status` + `DsShardState::{Attached,Attaching,Detaching,Expired,Detached}`）、`src/eloq_store.cpp:922-1058`（`AttachShard`）、`1060-1145`（`DetachShard`）、`661-684`（`Start` = StartInstance + AttachShard）

## 现状
Rust 模型：`Runtime` 起 N 个 `Shard` 线程，每 shard 一个 `Store<MeteredIo<CompioStore>>`；请求按 partition hash 路由到 shard；attach 即向该 shard 队列发 `Request::AttachShard{branch, shard_id, term}`：
- `claim_term(branch, shard_id, term)` **无条件写本地 CURRENT_TERM 文件**（Local 模式也一样）；
- `prepare_attach` 清空 Store 全部 table 状态并切 manifest；
- detach 只驱逐本 shard 的 table 缓存，**保留 branch/term**（`fkv_shard/src/lib.rs:609-625`）；
- 无 Store 级 Status 原子机（Running/Stopping/Starting），无 ds_shard 槽位状态机（Attaching/Detaching/Expired），无「同 Store 托管多个 data store shard」概念（基线 commit 82b774c 的标题即 `host several data store shards on one store (#502)`），无 `partition_filter`（sto-57），`AttachShard` 不拒绝与已 attached shard 冲突的再次 attach（除本地旧 term 快速失败）。

## C++ 实现
`EloqStore::StartInstance`（`eloq_store.cpp:686-790+`）：ValidateOption → 状态机 CAS Stopped→Starting→Running；`store_mode_` 原子派生（Cloud/Standby 各自建 CloudStorageService/StandbyService，Local 清空两者）。
`AttachShard`（922-1058）：
- **Local 模式 term 强制为 0**（931-932 行 `mode == Local ? 0 : term`），不碰 term 文件；
- `ds_shard_mux_` 串行化下检查：已有其它非 Detached shard 且非 Cloud 模式 → 拒绝（Local/Standby 单 shard 所有权）；Cloud 托管多个必须带 `partition_filter`（962-980 行）；
- slot 状态 Attached→Attaching→Attached CAS，Cloud 下经 `AttachShardRequest` 到 worker shard 做 term 文件 CAS；
- `DetachShard`（1060-1145）：状态置 Detaching（期间拒绝请求），向**每个** worker shard 各发一个 `DetachShardRequest` 等待全部排空（1103-1133），随后 slot 复位 Detached、**term 归零**（1140 行）；
- `MarkShardExpired`（1162-1176）把 Attached→Expired（term 过期由外部驱动），`HostedShards` 列出 Attached/Expired。

## 差异与风险
- **Local 模式产生 C++ 不存在的磁盘副作用**：Rust 每次 attach 在 store 根目录写 `CURRENT_TERM_<branch>_<shard>` 文件；C++ Local 从不写 term 文件（term 仅云/standby 概念）。交叉打开同一目录时 Rust 侧多出文件（不破坏兼容，但「Local 模式零副作用」的 L1 互操作声明失真）；
- **detach 语义**：C++ detach 排空全部 worker shard 上该 shard 的请求并把 term 归零；Rust detach 只驱逐 table 缓存、保留 term——同一 `shard_id` 以**更旧 term** 重新 attach 时 Rust 拒绝（`fkv_shard:588`），C++ 本地无此约束（term=0 恒定），云模式以磁盘 term 文件为准。混合语义在交替打开场景（D3）下的行为与 C++ 不同源；
- **多 shard 托管缺失**：C++ 基线能力「一个 Store 托管多个 data store shard + partition_filter」Rust 无对应物；`fkv_api::Request` 也不携带 per-request 的 `DsShardId`（C++ `KvRequest::SetDsShard`，`eloq_store.h:160-168`），路由只用 partition hash，无法表达「请求属于哪个 ds_shard」；
- **生命周期健壮性**：Rust 无 Stopping 拒绝新 attach、无「attach 中掉电」的 Attaching 中间态、无 Expired 快速失败的持久化依据（Expired 只靠内存 flag），stop 与 in-flight 请求的竞争依赖 oneshot drop 语义兜底（见 sto-62）。

## 对齐建议
- `Runtime` 增加 Status 原子机（Stopped/Starting/Running/Stopping）与 CAS 协议（对齐 StartInstance 686-790）；
- `Shard` 增加 DsShardState 状态机（Attached/Attaching/Detaching/Expired/Detached）+ `Attaching`/`Detaching` 期间拒绝普通请求；
- Local 模式 attach 时跳过 `claim_term`（或 term 强制 0 且不落盘），对齐 `eloq_store.cpp:931-932`；
- `Request` 增加可选 `ds_shard: Option<u32>`，运行时按 partition_filter 路由，并在 attach 时校验同 ds_shard 不重复托管（除 Cloud）；
- detach 请求广播到所有 shard 线程等待排空（对齐 DetachShardRequest fanout），term 归零语义统一。

## 性能优化建议
- detach 的 fanout 等待可用现有 `join_all` 并发化（D64 已有跨 shard fanout 先例）。

## 严重度
P1（行为不一致/功能缺失）

## 审查结论（2026-11-04）

逐条核验后判定：**部分合理，已拆分为 3 个子 issue，其中 1 个当场落地，2 个留 todo**。

### 已落地（sto-61a）

- `ShardState` 的 `attached: bool` + `expired: bool` 改为 `DsShardState` 枚举
  （`Detached` / `Attached` / `Expired`），准入、attach、detach 全部走 `match`。
- attach / detach 非法转换返回错误：已托管时换 branch / 换 shard / 提 term 均
  `InvalidArgs`（对齐 C++ `AttachShard` 要求 slot `Detached`）；`shard_id` 不匹配的
  detach 亦 `InvalidArgs`；同 shard/branch/term 重复 attach 与 `Detached` 下重复
  detach 幂等。
- 新增 `crate/engine/fkv_shard/tests/lifecycle.rs` 覆盖幂等与非法转换；更新
  `fkv_runtime/tests/runtime.rs` 的换 branch 用例（需先 detach）。

### 判定为不做 / 转 todo

- **Runtime Status 原子机（Stopped/Starting/Running/Stopping）**：Rust `Runtime` 的
  `shutdown(self)` 消费所有权，构造即 Running，API 上不存在 Starting / Stopping
  窗口，CAS 无落点。多 shard 生命周期随 sto-61b 一起设计。
- **多 ds_shard 托管 + per-request `ds_shard` + `partition_filter` + detach fanout**：
  需要改 `fkv_api::Request` 契约与 runtime 路由，属大改，转
  `issue/todo/sto-61b-multi-ds-shard-hosting-and-request-routing.md`。
- **Local 模式 term 文件 / detach term 归零**：与 Rust 现有 `put_with_term` Local
  fencing 契约冲突（`fkv_runtime::detach_and_term_validation` 依赖），需要先决策，
  转 `issue/todo/sto-61c-local-mode-term-file-and-detach-term-reset.md`。
- **C++ `Attaching` / `Detaching` 中间态**：shard 线程串行处理控制请求，中间态不跨
  请求可见；保留为文档说明，不引入伪状态。

### 验证

- `cargo nextest run -p fkv_shard -p fkv_store -p fkv_runtime --no-fail-fast`：
  161 passed, 1 skipped。
- `cargo clippy -p fkv_shard -p fkv_store -p fkv_runtime --no-deps --all-features
  --all-targets -- -D warnings`：零警告。

## DONE（2026-11-04）

## 复审（2026-11-05 r6）

**通过（拆分归口闭合）。** 子项核实：sto-61a（DsShardState 状态机，done→本次复审通过）、sto-61c（Local term 文件决策 A，done→通过）；Runtime Status 原子机与 Attaching/Detaching 中间态两条「不做」的判定理由（shutdown 消费所有权、shard 线程串行处理控制请求）在 r6 复核 eng-32b 时仍成立——注意 eng-32b 落地读 spawn 后，`dispatch` 写/控制臂仍走串行 `store_mut()`（`fkv_shard/src/lib.rs:1165/1175`），中间态不跨请求可见的前提未破坏。sto-61b（多 ds_shard 托管）留 todo 由在途会话承接，不算缺失。
