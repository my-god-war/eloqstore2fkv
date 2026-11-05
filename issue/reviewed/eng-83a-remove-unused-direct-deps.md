# eng-83a 删除 5 个未使用的 crate 直接依赖

来源：`issue/todo/eng-83-unused-dependencies-shear.md`（审查结论 2026-11-04）。

## 位置

| crate | 依赖 | Cargo.toml 行 |
|---|---|---|
| `fkv_api` | `fkv_error` | `crate/engine/fkv_api/Cargo.toml:12` |
| `fkv_large` | `pollster`（dev） | `crate/core/fkv_large/Cargo.toml:25` |
| `fkv_object_s3` | `futures-channel` | `crate/io/fkv_object_s3/Cargo.toml:15` |
| `fkv_task_gc` | `fkv_recovery` | `crate/engine/fkv_task_gc/Cargo.toml:15` |
| `fkv_task_gc` | `fkv_manifest` | `crate/engine/fkv_task_gc/Cargo.toml:16` |

## 方案

1. 用 `cargo remove` 删除（按仓库约定，不手改 Cargo.toml）：
   - `cargo remove -p fkv_api fkv_error`
   - `cargo remove -p fkv_large --dev pollster`
   - `cargo remove -p fkv_object_s3 futures-channel`
   - `cargo remove -p fkv_task_gc fkv_recovery fkv_manifest`
2. 保留 `fkv_object_s3/futures`（`StreamExt` 等在用），保留所有 `workspace.dependencies`
   预留声明，不动版本表。
3. 若 `fkv_task_gc` 出现类型推断错误（E0282）才单独补 `fkv_recovery`，并注明原因。

## 验收

- `cargo check --workspace --all-targets --all-features` 通过
- `cargo nextest run --workspace --all-features --no-fail-fast` 通过
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 零警告
- `cargo machete` 不再报告上述 4 项，`rg pollster crate/core/fkv_large/` 只剩无匹配

## DONE（2026-11-04）

按方案用 `cargo remove` 删除 5 项（`fkv_api/fkv_error`、`fkv_large/pollster`(dev)、
`fkv_object_s3/futures-channel`、`fkv_task_gc/fkv_recovery`、`fkv_task_gc/fkv_manifest`）。
`fkv_task_gc` 未出现 E0282，无需补回 `fkv_recovery`；`futures` 与 workspace 预留声明未动。

验证：

- `cargo check --workspace --all-targets --all-features` 通过
- `cargo nextest run --workspace --all-features --no-fail-fast`：421 passed, 1 skipped
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 零警告
- `cargo machete`：无未使用依赖；`rg pollster crate/core/fkv_large/` 无匹配

遗留：CI 门禁（`cargo shear --deny-warnings` / `./sh/unused.js`）不在本 issue 范围，
可并入既有 `issue/todo/eng-33b-orphan-guard-ci.md`。

## 复审（2026-11-05 r6）

通过。5 处依赖已清：`fkv_api/Cargo.toml` 无 fkv_error、`fkv_large` 无 pollster（dev 亦无）、`fkv_object_s3` 无 futures-channel、`fkv_task_gc` 无 fkv_recovery/fkv_manifest（grep 逐个确认）。
