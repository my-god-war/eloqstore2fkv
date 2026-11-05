# rev-33 group-commit Put 路径 term 失配不回标 Expired，分片继续服务过期 term

## 位置
- Rust: `crate/engine/fkv_shard/src/lib.rs:1340-1350`（`run_put_group` 的 term 校验）、`crate/engine/fkv_shard/src/lib.rs:1529-1537`（`dispatch` 中单请求 Put 的对照行为）
- C++: `src/storage/shard.cpp:1416-1421`（`Shard::OnTaskFinished`：任务返回 `ExpiredTerm` 即 `store_->MarkShardExpired(req_shard)`）

## 现状
`drain_write_gate` 把同分区、同 term 的连续高优先 `Put` 合并成一次 `Store::put`（group commit）。
`run_put_group` 内检测到请求 term 与 `state.term` 失配时只把整组回执成
`Err(KvError::ExpiredTerm)`，**不写 `state.lifecycle = DsShardState::Expired`**：

```rust
// run_put_group（lib.rs:1345-1350）
if result.is_ok()
  && let Some(term) = term
  && state.term != Some(term)
{
  result = Err(KvError::ExpiredTerm);   // 仅回执错误，不改 lifecycle
}
```

而单请求（组大小为 1）走 `run_gated_write → process_envelope → handle → dispatch`，
`dispatch` 的 Put 臂（lib.rs:1530-1536）会先 `state.lifecycle = DsShardState::Expired`
再返回 `ExpiredTerm`。`PutLarge` 同理（`put_term` 只匹配 `Put`，PutLarge 永远走单请求路径）。

即：**同一错误（stale term 的 Put）走不走 group-commit 快路径，分片生命周期状态不同。**

## C++ 实现
C++ 的失配标记不在请求分发处，而在任务完成处：任何任务（含 BatchWrite）返回
`KvError::ExpiredTerm`，`OnTaskFinished` 都执行
`store_->MarkShardExpired(req_shard)`（shard.cpp:1416-1421），整片转入 `Expired`，
其后除 attach/detach/shutdown 外全部请求经 `AdmitRequest` 快速失败 `ExpiredTerm`
（shard.cpp:658-705）。C++ 没有 group commit，因此不存在"另一条路径漏标记"的可能。

## 差异与风险
- group-commit 快路径检测到 term 被接管后，分片仍保持 `Attached`：后续不带 term 的
  `Get`/`Scan`/`Floor`/`Reopen` 继续被放行，继续对旧 term 的 mapping 读写。
- 这正是 fencing 要封死的窗口：更高 term 已接管该分区后，旧 shard 仍可能把旧状态的
  读返回给调用方，或（更糟）放行的后续单请求写再次逐个报 `ExpiredTerm` 才触发标记。
- 触发条件不苛刻：stale term 的 `Put` 在队列里凑够 ≥2 条同 term 即走该路径；弹性
  owner 迁移 + 写突发场景下可复现。

## 对齐建议
在 `run_put_group` 的 term 失配分支补 `state.lifecycle = DsShardState::Expired;`
（与 `dispatch` Put 臂一致），并把该判定抽成单一函数供两条路径共用，避免再次分叉。
更彻底的做法：让 `run_put_group` 复用 `handle`/`dispatch` 的准入语义，错误分类
（ExpiredTerm/NotRunning）由单点产生。

## 严重度
P1

## 核验记录
- 通读 `fkv_shard/src/lib.rs` 全文：`run_put_group`（1318-1377）、`drain_write_gate`
  （1235-1280）、`dispatch` Put/PutLarge 臂（1529-1566）、`admit`（1062-1075）、
  `DsShardState::Expired` 定义与 `admit_attach` 语义。
- 通读 C++ `src/storage/shard.cpp`：`OnTaskFinished`（1355-1532，MarkShardExpired 在
  1416-1421）、`AdmitRequest`（658-705）。
- 确认 group 路径绕开 `process_envelope`/`handle`，且 `run_put_group` 全文无任何
  `lifecycle` 写操作；`PutLarge` 不进 group（`put_term` 只匹配 `Put`）。

## 甄别（2026-11-05 r6）

**结论：意见已过时，驳回。** 意见基于并行会话合入前的旧代码快照；当前源码中
`run_put_group` 的 term 失配路径**已经**写 `state.lifecycle = DsShardState::Expired`，
且语义完整（标记 + 整组回执），与单条 Put 路径逐句对齐。

当前代码证据（`crate/engine/fkv_shard/src/lib.rs`，行号为本轮实读）：

- `run_put_group`（lib.rs:1656-1730）term 失配分支 lib.rs:1692-1699：
  ```rust
  if result.is_ok()
    && let Some(term) = term
    && state.term != Some(term)
  {
    // 与单条 Put 路径对齐：stale term 之后整个 shard 进入 Expired 快速失败。
    state.lifecycle = DsShardState::Expired;
    result = Err(KvError::ExpiredTerm);
  }
  ```
  随后 lib.rs:1720-1724 对全部存活 envelope 回执该 `Err(ExpiredTerm)`（标记+回执齐全）。
- 对照单条路径：`dispatch` Put 臂 lib.rs:2194-2203（2198 行写 Expired、2199 行返
  `ExpiredTerm`），PutLarge 臂 lib.rs:2204-2210（2208 行）同样标记。两路径均先经
  `admit`（group：lib.rs:1688；单条：`handle` lib.rs:1404-1408），`admit` 对
  `Expired` 快速失败返 `ExpiredTerm`（lib.rs:1386-1390），后续不带 term 的
  Get/Scan 不会再被放行——意见中「分片继续服务过期 term」的风险不成立。
- 意见建议的修复（补 `state.lifecycle = DsShardState::Expired` 与 dispatch 对齐）
  正是当前代码已实现的内容，连对齐注释都在；建议中的「抽单一函数」仅为重构偏好，
  非正确性问题。
- 行号漂移旁证：意见引 `run_put_group` 于 1318-1377，当前在 1656-1730；r6 lane
  复审记录（issue/reviewed/00-r6-store-write.md:62-66）亦注明并行会话期间行号多次
  漂移，且实读确认该标记已存在。
- C++ 侧维持意见原引无误：`src/storage/shard.cpp:1416-1420`
  `if (err == KvError::ExpiredTerm) → store_->MarkShardExpired(req_shard)`。
