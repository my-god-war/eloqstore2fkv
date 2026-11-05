# r9b-03 P3：`Store::with_branch` 归一化失败静默吞错——同族三 attach 入口两个拒绝一个放行，与 C++ InvalidArgs 相悖

## 现象（亲读）

`crate/engine/fkv_store/src/lib.rs:934-943`：

```rust
pub fn with_branch(mut self, branch_name: impl Into<String>, term: u64) -> Self {
  let branch_name = branch_name.into();
  let normalized = normalize_branch_name(&branch_name)
    .map(|name| name.into_string())
    .unwrap_or(branch_name);          // ← 归一化失败：原样保留非法名
  self.io.set_file_identity(&normalized, term);
  self.branch_name = normalized;
  ...
}
```

`normalize_branch_name`（`fkv_types/src/lib.rs:243-256`）对空名、下划线、斜杠、
点等返回 `None`。`with_branch` 把 `None` 回退为**原始输入**：调用
`with_branch("Feature_1", 0)` 后 Store 以含大写与下划线的名字运行——文件身份、
`branch_name`、后续 `refresh_branch_metadata` 写进 manifest 的
`BranchManifestMetadata.branch_name` 全部携带非法名，直到某个控制面入口
（如 `create_branch` :3431 对 `self.branch_name` 再归一化）才以 `InvalidArgs`
炸出，且炸点离错误源头隔了若干次写入。

同族入口行为分叉（同文件亲读）：

- `prepare_attach` :1893-1894：`normalize_branch_name(...).ok_or(KvError::InvalidArgs)?` ——拒绝；
- `adopt_attach` :1907-1908：同上拒绝；
- `open_branch` :1945：同上拒绝；
- `with_branch` :936-938：**静默放行**。

当前生产唯一调用点 `fkv_sdk/src/lib.rs:77` 传 `MAIN_BRANCH_NAME` 常量，测试
（`naming.rs:33/:85` 等）传合法名，故是潜在陷阱而非现行缺陷——但 builder
一旦被宿主以用户输入调用（SDK 单根入口的可见性使之很容易被这么用），非法
身份会静默落盘。

## C++ 对照

`src/tasks/background_write.cpp:616-621`（`CreateBranch`）与同文件
`DeleteBranch` 同款：`NormalizeBranchName` 结果为空即
`return KvError::InvalidArgs`。C++ 侧不存在「归一化失败沿用原名」的路径；
`include/common.h:76-105` 的 `NormalizeBranchName` 对非法字符（含下划线）
LOG(WARNING) 后返回空串，调用方全部 fail-fast。fkv 的 `with_branch` 是四个
入口里唯一的例外。

## 目标

`with_branch` 与同族对齐：归一化失败不再回退原名。builder 无 `Result` 通道，
两种做法任选：

1. 改签名返回 `Result<Self>`（调用点仅 fkv_sdk:77 与测试，波及面小）；
2. 或保持签名，失败时 `debug_assert!` + 回退 `MAIN_BRANCH_NAME` 并
   `log::warn!`（运行时可见）。

不要求改行为语义（合法名路径逐字节不变），只要求消除静默吞错。

## 验收

- 非法名（空 / 含 `_` / 含 `/`）进入 `with_branch` 时不再以原名静默生效：
  拒绝、告警回退或返回 `Err` 三者其一，且有测试断言该行为；
- `naming.rs` 既有用例不回归；`fkv_sdk` / `fkv_runtime` 编译零警告；
- `./sh/clippy.sh` 零警告。

## 核验记录

- 亲读 `fkv_store/src/lib.rs:934-943` 与三个同族入口（:1893/:1907/:1945）；
- 亲读 `fkv_types/src/lib.rs:243-262`（`normalize_branch_name` /
  `is_valid_branch_name`）确认 `None` 的触发集；
- `grep -rn "\.with_branch(" crate/` 逐一核对调用面：生产仅 `fkv_sdk:77`
  （常量），其余为测试合法名——立项为一致性 / 吞错问题，非现行数据损坏；
- 亲读 C++ `background_write.cpp:616-621` 与 `DeleteBranch` 的
  `NormalizeBranchName` 空名拒绝路径；
- 查重：r9a-01/02/03、r9c-01、r9e-01/02/03 均不涉本条；`naming.rs` 测试只
  覆盖合法名归一化，无非法名断言，非重复立项。

> 认领（2026-11-05 06:50，本会话子代理开发中）。lane：crate/engine/fkv_store。

## DONE（2026-11-05）

- 修复：`with_branch`（crate/engine/fkv_store/src/lib.rs，`fn with_branch`）归一化失败不再
  `.unwrap_or(原名)` 静默沿用。`Some` 合法名路径逐字节不变；`None` 路径按本 issue 方案 2
  收口——`debug_assert!(is_valid_branch_name(...))`（debug / 测试构建立即炸出，fail-fast）
  加 `log::warn!` 回退 `MAIN_BRANCH_NAME`（release 运行时可见），文件身份与 manifest 元数据
  不再携带非法名落盘。三个同族入口（prepare_attach / adopt_attach / open_branch）的
  `InvalidArgs` 拒绝路径未动；sto-69（done）的 fkv_shard 侧 `state.branch` 归一走
  `prepare_attach`，语义不受影响（`cargo nextest run -p fkv_shard` 81 passed 复核）。
- 未选方案 1（改 `Result<Self>` 签名）的原因：复核调用面发现除 `fkv_sdk:77` 外，
  `fkv_task_prewarm/tests/prewarm.rs`、`fkv_cloud/tests/*` 也调 `with_branch`，签名改动
  波及三个 lane 外 crate，按纪律不可触碰；方案 2 可在 lane 内完全收口。
- 测试（tests/naming.rs 新增 `with_branch_rejects_unnormalized_names`）：覆盖空名 /
  `Feature_1` / `feature/1` / `feature.1` 四种 `normalize_branch_name` 返回 `None` 的形态，
  `catch_unwind` 加 `cfg!(debug_assertions)` 双 profile 断言——debug 拒绝（panic）、
  release 回退不 panic。不用裸 `#[should_panic]` 是因其只在 debug 成立，`--release` 下必红。
  合法名行为不变由既有用例（`Feature-1` 归一为 `_feature-1_7` 文件名后缀）继续覆盖。
- 验证：`cargo nextest run -p fkv_store -p fkv_task_write --no-fail-fast` 202 passed（新增
  用例在 debug 与 `--release` 两 profile 均过）；`cargo clippy -p fkv_store --all-features
  --all-targets -D warnings` 零警告；下游 `cargo nextest run -p fkv_sdk -p fkv_runtime
  -p fkv_task_prewarm` 90 passed、`-p fkv_cloud` 82 passed；`cargo clippy -p fkv_sdk
  -p fkv_runtime -p fkv_task_prewarm --all-targets --all-features -D warnings` 零警告；
  `cargo +nightly fmt --check -p fkv_store` 通过。
- 备注：`cargo clippy -p fkv_cloud --all-targets -D warnings` 现存 2 个与本条无关的报错
  （`fkv_cloud/src/lib.rs:3625` `segment_marker_present` never used、`:3749` `Box::pin`
  local pinning），非本 lane 引入，留给 fkv_cloud owner。

## 复审（2026-11-05 v，lane A）

- 亲读复核：`fkv_store/src/lib.rs:949-968` `with_branch` 归一化失败分支改为 `debug_assert!(is_valid_branch_name(&branch_name), ...)` + `log::warn!` + 回退 `MAIN_BRANCH_NAME`（issue 方案 2），不再 `.unwrap_or(原名)`；`is_valid_branch_name`/`MAIN_BRANCH_NAME` 已导入（:35-36）。合法名路径逐字节不变（`Some(name) => name.into_string()`）。
- 测试：`fkv_store/tests/naming.rs:109` `with_branch_rejects_unnormalized_names` 覆盖 `""`/`Feature_1`/`feature/1`/`feature.1`，`catch_unwind` + `cfg!(debug_assertions)` 双 profile 断言（debug 拒绝、release 回退不 panic）。
- 在途提示：`fkv_store/src/lib.rs` mtime 07:49（近 5 分钟并行改动），`git diff` 仅动 `:2847` archive 分支（`write_snapshot_manifest` 失败清表），未触 `with_branch`，本条锚点稳定。
- 判定：完善。
