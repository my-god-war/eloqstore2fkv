# sto-69 P2：`AttachShard` 存的 `state.branch` 未 normalize，与 store 内部归一分叉；外加 `"main"` 字面量

来源：第 5 轮 runtime/cloud lane 侦查，协调者亲验。

## 证据（2026-11-05 核验）

`crate/engine/fkv_shard/src/lib.rs:1537-1542`：

```rust
state.store.claim_term(branch, *shard_id, *term).await?;
state.store.prepare_attach(branch, *term)?;   // 内部 normalize："Main" → "main"
state.lifecycle = DsShardState::Attached;
state.term = Some(*term);
state.branch = branch.clone();                 // 原样未归一
```

下游用字面量比较（`:622`、`:1017`）：

```rust
if branch == "main" { ... }
&& state.branch == "main";
```

`normalize_branch_name`（`fkv_types/src/lib.rs:212-225`）把大写折叠为小写。
attach `"Main"`：store 侧按 main 活跃模式装配，但 `state.branch=="Main" !=
"main"` → `ensure_table_ready` 走 `open_branch(tbl, "Main", 0)` 分支——同一分支
不同字符串形态行为不一致；detach 后 `admit_attach` 的 same_owner 按原样比较，
"Main"/"main" 视作不同 owner，状态机被字符串形态碎片化。

另：`fkv_cloud` 今天的 diff 刚把同款 `"main"` 字面量换成 `MAIN_BRANCH_NAME`
常量，shard 是漏网点（eng-71 单一定义收尾）。

## 目标

1. attach 时 `state.branch = normalize_branch_name(branch)`（或入口统一归一后
   再进状态机），一处定义。
2. `== "main"` 字面量替换为 `MAIN_BRANCH_NAME`。

## 验收

- 测试：attach `"Main"` 与 attach `"main"` 行为完全一致（表打开模式、
  same_owner 判定）；grep 无裸 `"main"` 字面量。
- `cargo nextest run -p fkv_shard`。

## 依赖 / 重叠

- sto-61/61a（生命周期状态机，done）之上的字符串形态漏洞；eng-71（done）收尾。
- `fkv_shard/src/lib.rs` 是并行会话热点，开发前对齐 owner。

## 核验记录

- 本人实读：`fkv_shard/src/lib.rs:1533-1546`、grep `== "main"`。
- 子代理摘录（未复核）：`normalize_branch_name` 行号、`ensure_table_ready`
  分支细节。

## 审查结论（2026-11-04，协调者）

**成立（P2 命名一致性）**，已修：

- 两处 `"main"` 字面量（`ensure_table_ready` 的 `branch == "main"`、
  `prepare_table` 的 `state.branch == "main"`）改用 `MAIN_BRANCH_NAME`。
- `Request::AttachShard` 的分支名**不再直接落库**：先过 `normalize_branch_name`
  （小写、仅 `[a-z0-9-]`），非法名 `InvalidArgs`。不归一的后果（实测）：`"MAIN"`
  会写进 `state.branch`、并落 `CURRENT_TERM_MAIN_0`，与 `MAIN_BRANCH_NAME` 的判定
  全面失配（`prepare_table` 因此多走一次 ensure + reopen），且与 cld-52 同属
  「同一 bucket 内名字分裂」。

## DONE（2026-11-04）

- 改动：`crate/engine/fkv_shard/src/lib.rs`（imports、2 处常量替换、AttachShard 归一化）；
- 新增 `fkv_shard::backend::attach_normalizes_branch_name`：`"MAIN"` attach 成功后，
  用 `"main"` 同 shard/term 再 attach 必须**幂等成功**（不归一时 `admit_attach`
  因 branch 不等报 `InvalidArgs`）；`"bad/branch"` 直接拒绝；
- **反向验证**：把归一化那行换成 `let branch = branch.clone();` 后该测试 **FAIL**
  （`unwrap()` on `Err(InvalidArgs)`），改回即 PASS；
- `cargo nextest run -p fkv_shard` 31 passed。

## 复审（2026-11-05 r6）

**通过。** 核实：`fkv_shard/src/lib.rs:2379` `normalize_branch_name(branch).ok_or(InvalidArgs)?`（注释点名 sto-69 与 CURRENT_TERM_MAIN_0 后果）；:2402 `state.branch = branch.into()`（归一后落状态）；:874 `branch == MAIN_BRANCH_NAME`（裸 "main" 字面量已清，:1386 亦然）。测试 `tests/backend.rs:120` `attach_normalizes_branch_name`：:137-147 "MAIN" 后 "main" 同参幂等成功（不归一必 InvalidArgs）、:149-158 "bad/branch" 拒绝——断言实质覆盖。
