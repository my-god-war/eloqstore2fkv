# sto-64a Cloud 采纳远端当前 manifest（只清本地，不删远端）

## 来源
`issue/new/sto-64-missing-installexternalsnapshot-reopen-adopt-remote-state.md`
审查结论（2026-11-04）。

## 位置
- `crate/io/fkv_io/src/lib.rs`：`ManifestIo`
- `crate/engine/fkv_cloud/src/lib.rs:1354-1370`：`read_manifest` / `drop_manifest`
- `crate/foundation/fkv_metrics/src/lib.rs:482`：`MeteredIo` 必须显式转发新原语
- `crate/engine/fkv_shard/src/backend.rs:207`：`StoreBackend` 同上
- `crate/engine/fkv_store/src/lib.rs:1530`：`reopen` / `install_replay_state`
- `crate/engine/fkv_api/src/lib.rs`：`Request::ReopenExternal` + 回执
- `crate/engine/fkv_shard/src/lib.rs:1360`：`Reopen` 分派

## 现状
- Cloud 本地 cache 命中即返回，外部换主后不会重新拉远端 manifest。
- 无「只删本地 manifest」原语，Cloud 的 `drop_manifest` 会删远端对象。
- Cloud / StandbyReplica 的 `open_empty` 问题已由 eng-39c 修复；tag/archive 重开归 12a。

## 方案
1. `ManifestIo` 增默认方法：
   - `refresh_manifest(tbl)`：默认 no-op（Local/Mem 无远端）。
   - `drop_local_manifest(tbl)`：默认回落 `drop_manifest`（Local/Mem 无远端对象）。
   `MeteredIo` / `StoreBackend` 显式转发，避免默认 no-op 静默丢行为。
2. `CloudStore` 覆写：`refresh_manifest` 无条件 `remote.get(manifest_key)` 后
   `local.switch_manifest`，缺失返回 `ResourceMissing`；`drop_local_manifest`
   只删 `self.local`。
3. `Store::install_external_snapshot(tbl) -> Result<InstallOutcome>`：
   refresh → `reopen`（复用 replay + `install_replay_state`）；
   `ResourceMissing` → 只删本地 manifest + `evict_table`，返回 `WaitingRemote`，
   绝不以本地空状态起跑。
4. `Request::ReopenExternal { tbl }` / `Response::ReopenExternal(InstallOutcome)`；
   shard 仅 `reopen_on_missing`（Cloud/StandbyReplica）放行，其余 `InvalidArgs`；
   StandbyReplica 先走 `sync_standby_partition`；`op_of` 记 `Op::Reopen`。

## 验收
- `fkv_cloud`：本地无 cache 安装成功；本地旧 manifest 被远端新状态覆盖；
  远端缺失 → `WaitingRemote` 且只删本地（远端对象仍在）。
- `fkv_shard`：Local 模式返回 `InvalidArgs`；Cloud 模式缺 manifest 返回
  `WaitingRemote`，之后远端出现时读请求仍能拿到数据。
- `cargo nextest run -p fkv_api -p fkv_shard -p fkv_store -p fkv_cloud -p fkv_runtime --no-fail-fast`。
- clippy 上述 crate `--no-deps --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

### 实现要点
1. `ManifestIo` 增 `refresh_manifest`（默认 no-op）与 `drop_local_manifest`
   （默认回落 `drop_manifest`）；`MeteredIo`、`StoreBackend` 显式转发。
2. `CloudStore::refresh_manifest`：无条件 `remote.get` 当前 active manifest；
   先 `invalidate_local_files`（摘 resident 记账 / file_range_map，删本地
   data+segment cache），远端缺失返回 `ResourceMissing`；成功则
   `local.switch_manifest`。`drop_local_manifest` 只删本地。
   把 `CloudStore::evict_table` 的状态清理抽成 `forget_table_cache` 复用。
3. `fkv_types::InstallOutcome { Installed, WaitingRemote }`；
   `Store::install_external_snapshot`：refresh → 复用 `reopen`；
   `ResourceMissing`（含 refresh 后 manifest 消失）→ 只删本地 manifest +
   `evict_table`，返回 `WaitingRemote`。
4. `Request::ReopenExternal { tbl }` / `Response::ReopenExternal(InstallOutcome)`；
   shard 仅 `reopen_on_missing` 放行（Local/Mem 返回 `InvalidArgs`），
   StandbyReplica 先 `sync_standby_partition`，`op_of` 记 `Op::Reopen`。

### 改动文件
- `crate/io/fkv_io/src/lib.rs`
- `crate/engine/fkv_cloud/src/lib.rs`、`crate/engine/fkv_cloud/tests/cloud.rs`
- `crate/foundation/fkv_metrics/src/lib.rs`
- `crate/foundation/fkv_types/src/lib.rs`
- `crate/engine/fkv_store/src/lib.rs`
- `crate/engine/fkv_api/src/lib.rs`
- `crate/engine/fkv_shard/src/{lib.rs,backend.rs}`、`crate/engine/fkv_shard/tests/cloud_reopen.rs`

### 验证
- `cargo nextest run -p fkv_api -p fkv_shard -p fkv_store -p fkv_runtime -p fkv_cloud
  --no-fail-fast`：189 passed / 1 skipped。
- clippy `fkv_api fkv_shard fkv_store fkv_cloud fkv_metrics fkv_types fkv_io`
  `--no-deps --all-features --all-targets -- -D warnings`：0 警告。
- 新用例：Cloud 采纳远端旧/新 manifest、远端缺失只清本地、drop_local 保留远端；
  shard Local 拒绝、Cloud `WaitingRemote` 后读到延迟 manifest、已有 manifest 安装。

### 遗留
- tag 语义（按归档 tag 安装指定快照）归 `issue/todo/12a-per-table-request-gaps.md`。
- 真实 S3 端到端未跑（沿用 `fkv_cloud/tests/s3_rustfs.rs` 环境要求）。

## 复审（2026-11-05 r6）

**通过。** 核实：

- `fkv_io/src/lib.rs` `ManifestIo` 增 `refresh_manifest`（默认 no-op）/`drop_local_manifest`（默认回落 drop_manifest）；`fkv_shard/src/backend.rs:264-277` `StoreBackend` 显式转发两原语。
- `fkv_cloud/src/lib.rs:1809` `refresh_manifest`（无条件回源 + `invalidate_local_files` + `switch_manifest`）、:1824 `drop_local_manifest` 只删本地。
- `Request::ReopenExternal`：`fkv_shard/src/lib.rs:1053` op 记 Op::Reopen；shard 仅 `reopen_on_missing` 放行，Local/Mem `InvalidArgs`。
- 测试 `fkv_shard/tests/cloud_reopen.rs:237` `local_reopen_external_rejected`（:247 `InvalidArgs`）、:256 `cloud_reopen_external_waits_then_reads_late_manifest`（:266 `WaitingRemote`）、:283 `cloud_reopen_external_installs_existing_manifest`；fkv_cloud 侧新用例见 12c/64a md 引用的 cloud.rs/term_key.rs。
