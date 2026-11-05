# sto-61c Local 模式 term 文件副作用与 detach term 归零

## 来源

`issue/new/sto-61-store-lifecycle-no-state-machine-no-multi-shard-hosting-attach-detach-divergence.md`
的 term 语义部分。与现有 `fkv_api` 请求契约耦合，需要单独决策，本 issue 不硬做。

## 位置

- Rust: `crate/engine/fkv_shard/src/lib.rs`（`AttachShard` 无条件 `claim_term`；
  `DetachShard` 保留 branch/term）、`crate/io/fkv_io_compio/src/term.rs`（
  `CURRENT_TERM_<branch>_<shard_id>` 落盘）、`crate/engine/fkv_store/src/lib.rs`
  （`claim_term` / `put_with_term`）
- C++: `src/eloq_store.cpp:926-932`（Local 模式 `shard_term = 0`，不碰 term 文件）、
  `1133-1141`（detach 时 `slot.term_.store(0)`）

## 现状与差异

- Rust Local 模式 attach 仍写 `CURRENT_TERM_<branch>_<shard_id>`，C++ Local 从不写
  term 文件（term 仅 Cloud / Standby 概念）。交叉打开同一目录会多出文件；C++ 不会因
  该文件失败，但「Local 零磁盘副作用」的 L1 声明不成立。
- Rust detach 保留 term（用于拒绝旧 term 重新 attach），C++ detach 把 term 归零。
- Rust 的 `Request::Put/PutLarge` 带 `Option<u64>` term，Local 模式下 `put_with_term`
  依赖 term 文件 fencing（`fkv_runtime::detach_and_term_validation` 覆盖）。

## 方案（二选一，需确认）

- **A. 保留 Rust Local fencing 契约**：文档化「Local 也使用 term 文件」为有意增强，
  不写 term 文件会削弱 `put_with_term` 语义。仅补文档与 ledger 说明。
- **B. 对齐 C++**：Local 模式 attach 强制 term = 0 且不写盘；`put_with_term` 在
  Local 下退化为不校验或返回 `InvalidArgs`；同步修改 `fkv_runtime` 的 term 测试与
  `fkv_api` 文档。改动面涉及请求契约，需与 sto-62 的 Request 契约一并评审。

推荐 A，除非明确要求与 C++ 行为逐字节一致。

## 验收

- [ ] 方案确认后：Local attach 不再产生/仍产生 `CURRENT_TERM_*` 的集成断言。
- [ ] detach 后 term 语义（保留或归零）有测试覆盖。
- [ ] `cargo nextest run -p fkv_shard -p fkv_store -p fkv_runtime` 全绿。

## DONE（2026-11-04）

判定：采用方案 A。保留 Local 模式写 `CURRENT_TERM_<branch>_<shard_id>` 作为 `Put{term}`
的 fencing 依据，不随后端分叉；C++ Local 不写 term 文件记为**有意差异**，不逐字节对齐。

落地：

- `crate/engine/fkv_shard/src/lib.rs`
  - `AttachShard`：注释明确 Local 也落 term 文件，同 term 幂等、低 term 由 CAS 拒绝。
  - `DetachShard`：运行期 `term` / `branch` 归零（对齐 C++ `slot.term_.store(0)`）；
    term 文件**保留**，旧 term 重新 attach 仍由持久 CAS 拒绝，不依赖内存状态。
- `crate/engine/fkv_shard/tests/lifecycle.rs`：新增
  `local_term_file_fences_and_detach_resets_state`，覆盖 attach 落盘、detach 后文件保留、
  detach 后旧 term attach 被拒、换 term 后旧数据可读、旧 term 写入失败。
- `doc/architecture/03-io-stack.md`：补「term 语义」段落（三后端一致性、detach 归零
  但不删文件、跨 term reopen 的 allocator bump）。

不变量未动：append-only 分配器只在 `install_replay_state` 按 term 变化 bump；
旧 term 数据文件按 file id 跨 term 可读；跨 term 映射未修改。

重叠评估：与 `12b` / `eng-39c`（均已 done）不重叠：那两条是 standby / cloud reopen
模式接线，本条只处理 Local term 文件与 detach 运行期状态。

保留未做：方案 B（Local 强制 term=0、`put_with_term` 退化为不校验 / `InvalidArgs`）
涉及 `fkv_api::Request` 契约，需与 `sto-62` 一并评审；本地 term 文件是已知差异，已在
文档说明。

验收：

- [x] Local attach 产生 `CURRENT_TERM_*` 的集成断言（lifecycle 测试）。
- [x] detach 后 term 语义（运行期归零、持久 term 保留）有测试覆盖。
- [x] `cargo nextest run -p fkv_store -p fkv_io_compio -p fkv_io -p fkv_runtime -p fkv_shard`
      212 passed / 1 skipped。

## 复审（2026-11-05 r6）

**通过（方案 A）。** 核实：

- `fkv_shard/src/lib.rs:2391-2394` attach 注释明示「Local 同样落 CURRENT_TERM，C++ Local 不写属已知差异（sto-61c）」；:2414-2419 detach 运行期 `term=None`/`branch.clear()`（对齐 C++ slot.term 归零），term 文件保留由持久 CAS 拒绝旧 term。
- 测试 `tests/lifecycle.rs:188` `local_term_file_fences_and_detach_resets_state`：:195 attach 落盘 "3"、:200 detach 后文件仍 "3"、:208 旧 term 2 attach `ExpiredTerm`、:216 跨 term 旧数据可读、:221 旧 term 写入失败——五段断言完整覆盖验收两条。
- `doc/architecture/03-io-stack.md:55` 「term 语义」段落存在。
