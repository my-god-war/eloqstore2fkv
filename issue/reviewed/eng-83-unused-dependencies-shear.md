# eng-83 unused.js 扫描：5 个未使用依赖可删除（2 个 dev-dependency、3 个传递依赖误留）

## 来源

`./sh/unused.js` 的 `cargo-shear` 段输出 5 个 `unused_dependency`，逐项核对源码后确认成立：

| crate | 依赖 | 位置 | 证据 |
|---|---|---|---|
| `fkv_api` | `fkv_error` | `crate/engine/fkv_api/Cargo.toml:12` | `fkv_api/src` 只 re-export `fkv_btree::BatchEntry` / `fkv_types`，无 `fkv_error` 引用 |
| `fkv_large` | `pollster` | `crate/core/fkv_large/Cargo.toml:25`（dev-dependency） | `fkv_large/tests` 全部用 `compio::runtime::Runtime::block_on`；`pollster` 只出现在 `fkv_object` / `fkv_standby` / `fkv_io_mem` 自己的 dev-deps |
| `fkv_object_s3` | `futures-channel` | `crate/io/fkv_object_s3/Cargo.toml:15` | 源码用 `futures::channel::oneshot`（`src/lib.rs:30,184,561`），`futures` 已 re-export，直接依赖 `futures-channel` 冗余 |
| `fkv_task_gc` | `fkv_recovery` | `crate/engine/fkv_task_gc/Cargo.toml:15` | `replay_manifest_bytes` 的返回类型由 `fkv_task_reopen` 透传，源码不直接命名 `fkv_recovery::*` |
| `fkv_task_gc` | `fkv_manifest` | `crate/engine/fkv_task_gc/Cargo.toml:16` | 源码只用 `fkv_task_reopen::replay_manifest_bytes`，不直接引用 manifest 类型/函数 |

## 现状

- `fkv_api` 的 `fkv_error` 是早期“Request/Response 错误码”预留；现在错误码只在 shard/store 层使用，API 层是纯数据枚举。
- `fkv_object_s3` 的 `futures-channel` 与 `futures` 重复：`futures` 的 `channel::oneshot` 已覆盖，删直接依赖即可。
- `fkv_large` 的 `pollster` 是复制其他 crate dev-deps 时带进来的；该 crate 的测试已统一 compio runtime。
- `fkv_task_gc` 的两个依赖是“实现 `manifest_retained_file_page_set` 时可能想过直接用 recovery/manifest”，最终只调 `fkv_task_reopen`，属于传递依赖误留。
- 这些依赖不影响行为，但会放大编译图、触发 `cargo shear` 持续告警；未使用依赖也会误导后续开发者以为存在跨层依赖。

## 建议

1. 手工删除 5 行（比 `cargo shear --fix` 更可控，避免工具顺手改动 workspace 版本声明）：
   - `crate/engine/fkv_api/Cargo.toml:12`
   - `crate/core/fkv_large/Cargo.toml:25`
   - `crate/io/fkv_object_s3/Cargo.toml:15`
   - `crate/engine/fkv_task_gc/Cargo.toml:15-16`
2. 删除后跑 `cargo check --workspace --all-targets` 与 `./sh/clippy.sh`；
   `fkv_task_gc` 若因类型推断出现 E0282，再单独补 `fkv_recovery` 并注释原因。
3. 在 CI 增加 `cargo shear --deny-warnings`（或 `./sh/unused.js`）作为门禁，
   防止 dev-deps 再次被复制粘贴带进来。
4. `fkv_object_s3` 的 `futures` 仍被使用（`StreamExt` 等），只删 `futures-channel`，
   不要整组删 `futures`。

## 严重度

P3（纯依赖清理；无行为风险，编译时间与依赖图收益）

## 核验记录（2026-11-04）

`cargo-shear` 输出 5 条；对每个依赖在对应 crate 的 `src/` 与 `tests/` 做
`grep -rn` 确认无引用。`fkv_large` 的 `pollster` 仅确认其他 crate 自用，本 crate 测试未用。

## 审查结论（2026-11-04）

判定：**成立**，5 项全部可删。

核验方式：

- `cargo machete` 输出 4 条（`fkv_api/fkv_error`、`fkv_object_s3/futures-channel`、
  `fkv_task_gc/fkv_manifest`、`fkv_task_gc/fkv_recovery`）；`fkv_large/pollster` 是
  dev-dependency，`cargo machete` 未报，但 `rg pollster crate/core/fkv_large/` 只命中
  `Cargo.toml` 本身，测试统一用 `compio::runtime::Runtime::block_on`，同样成立。
- 4 个 crate 的依赖均为**直接 path 声明**，不是 `workspace = true` 的 workspace 预留项，
  因此不适用 `todo.md` 的「未使用 workspace 依赖等实现时再启用」豁免。
- `fkv_object_s3` 的 `futures` 仍被使用，只删 `futures-channel`。
- `fkv_task_gc` 只调用 `fkv_task_reopen::replay_manifest_bytes`，返回类型由该函数透传，
  源码不命名 `fkv_recovery::*` / `fkv_manifest::*`。

结论：删除 5 个直接依赖，保留 `futures` 与全部 workspace 预留声明。按本 issue 拆出
`issue/todo/eng-83a-remove-unused-direct-deps.md` 执行。

## 复审（2026-11-05 r6b）

通过。

- 5 项依赖全部不在：`fkv_api/Cargo.toml` 无 `fkv_error`、`fkv_large/Cargo.toml` 无 `pollster`、`fkv_object_s3/Cargo.toml` 无 `futures-channel`、`fkv_task_gc/Cargo.toml` 无 `fkv_recovery`/`fkv_manifest`（逐文件 grep 0 命中）。
- 实施件 `eng-83a-remove-unused-direct-deps.md` 已在 `issue/reviewed/`；r6 各 lane 复核时依赖零残留，状态一致。
