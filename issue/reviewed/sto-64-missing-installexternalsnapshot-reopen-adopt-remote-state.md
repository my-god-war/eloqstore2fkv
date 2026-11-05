# sto-64 缺 InstallExternalSnapshot：云/standby reopen 不「采纳远端状态」，ResourceMissing 直接开空分区

## 位置
- Rust: 全仓无 `InstallExternalSnapshot`/external snapshot 任何匹配；reopen 路径 `crate/engine/fkv_store/src/lib.rs:1217-1234`（`auto_reopen_with_sleeper` 只 replay 本地 manifest）、`crate/engine/fkv_shard/src/lib.rs:278-298`（`ensure_table_ready` 在 ResourceMissing 时 `open_empty`）、`crate/engine/fkv_api/src/lib.rs:36-38`（`Request::Reopen` 无 tag/action）
- C++: `src/storage/page_manager.cpp:472-661`（`PageManager::InstallExternalSnapshot`）、`src/tasks/reopen_task.cpp:95-120`（reopen 任务按 tag 触发 install）、`include/eloq_store.h:648-660、703-720`（Reopen 请求的 `Action::Create/Install` + tag）、`include/storage/page_manager.h:66`

## 现状
Rust 的 reopen 语义：读**本地** manifest（`reopen_with_size`，`crate/engine/fkv_task_reopen/src/lib.rs:68-77`），找不到（ResourceMissing）则重试 N 次（`auto_reopen_retry_times`），重试耗尽后：main branch 走 `open_empty` 把分区当空表，branch 走 `open_branch` 报错。`fkv_cloud` 的本地 cache 层（D32）在 manifest 未下载时确实返回 ResourceMissing，因此「云 manifest 延迟出现」场景（D45 宣称已测）在 **CloudStore 库级**有测试，但 **shard 级请求路径**没有「从远端拉最新 manifest 并采纳」的入口。

## C++ 实现
`InstallExternalSnapshot(tbl, cow_meta, reopen_tag, &cleared_local_state)`（page_manager.cpp:472）：
- 仅 Cloud / StandbyReplica 模式（480-486 行）；
- 先 `RefreshManifest(tbl, reopen_tag)`（Cloud）/ standby 侧刷新（532-545 行）——拉到**远端当前 manifest**；
- NotFound/ResourceMissing：`DropLocalManifest`（只删本地！绝不删远端，553-564 行注释明确防误删），清空内存 RootMeta，`cleared_local_state=true`——分区改为等待下一次远端状态出现，而不是以本地空状态起跑（566-583 行）；
- 成功：`Replayer::Replay` + `GetMapper/GetSegmentMapper(expect_term=ProcessTerm)` 安装新 mapping/root/dict/branch file_ranges/`next_expire_ts`（592-655 行），原子 `UpdateRoot` 替换旧状态。
触发点是 Reopen 请求带 tag + Action（`reopen_task.cpp:95-120`，`eloq_store.h:648-660`）：外部系统（如 EloqDB 换主/重挂载）指定「用某 tag 的远端快照重开这个分区」。

## 差异与风险
- **副本以空状态起跑**：Cloud/StandbyReplica 下本地无 manifest 时，Rust `ensure_table_ready` 会 `open_empty` 并立即接受写入，本地写出的数据与主端状态完全分叉，之后远端 manifest 到达时也没有「替换」路径（Rust 只有 reopen 本地 manifest）——standby replica 场景是数据正确性问题（todo.md 宣称 standby 切换后数据一致，但那是「replica 先同步再 reopen」的顺序测试，不是「reopen 时主动采纳远端」的语义测试）；
- **`Request::Reopen` 无 tag/action**：无法表达「按 tag 安装远端快照」，`Action::Create/Install` 语义缺失（与 sto-62 的覆盖面缺口联动）；
- **local manifest 残留**：C++ 在采纳远端前 DropLocalManifest 防旧本地状态回灌；Rust 无对应动作，本地半残 manifest 与远端 manifest 并存时行为未定义（`manifest_retained_file_page_set` 会把两边都计入 retained，sto-54）。

## 对齐建议
- `fkv_store` 增加 `install_external_snapshot(tbl, tag) -> InstallOutcome`：
  1. 仅 Cloud/StandbyReplica 允许（模式门）；
  2. 经 `ManifestIo` 增加 `refresh_manifest(tbl, tag)` 原语（CloudStore/StandbyStore 实现拉远端 manifest 到本地）；
  3. NotFound/ResourceMissing → 删本地 manifest + 清内存 table 状态，返回「等待远端」；
  4. 成功 → replay + `install_replay_state`（对齐 sto-52 的分配器重建）原子替换；
- `Request::Reopen` 增加 `tag: Option<String>`，shard 侧按 tag 走 install 路径（对齐 Action::Install）；
- `ensure_table_ready` 在 Cloud/StandbyReplica 模式 ResourceMissing 时改为触发 install（重试耗尽后报错或等待），不得 `open_empty`。

## 性能优化建议
- 采纳远端时复用 `install_replay_state` 的 IndexCache 清空与字典重建路径，避免二次分配。

## 严重度
P1（功能缺失/云与 standby 模式数据分歧风险）

## 审查结论（2026-11-04）

判定：**部分合理，范围收缩**。

### 核验

- 第 8 行「`ensure_table_ready` 在 ResourceMissing 时 `open_empty`」已过期：
  `eng-39c`（done）已加 `ShardState.reopen_on_missing`，Cloud / StandbyReplica
  不再本地空表懒创建，`ResourceMissing` 挂 `ReopenQueue` 重试（driver/waiter 去重）。
- 「无 tag/action」重复：archive 按 tag 重开已在
  `issue/todo/12a-per-table-request-gaps.md:18,28` 登记为 `ReopenArchive { tag }`，
  本条不再另立 tag 通路。
- 「reopen 不采纳远端」大部分已具备：`CloudStore::read_manifest` 本地缺失时回源
  S3（`crate/engine/fkv_cloud/src/lib.rs:1354-1364`）；`StandbyReplica` 在
  `dispatch(Request::Reopen)` 前 `sync_standby_partition` 全量追一次 master。
- 真正缺口只剩两点：
  1. Cloud 本地 cache 有旧 manifest 时**永不回源**（`read_manifest` 先命中本地），
     无法采纳外部换主后的远端状态；
  2. 远端 manifest 不存在时没有「只删本地 manifest + 清内存状态」的原语
     （`CloudStore::drop_manifest` 会**连远端对象一起删**，与 C++ `DropLocalManifest`
     语义相反，不能直接复用）。

### 处置

保留最小闭环：`ManifestIo` 增 `refresh_manifest` / `drop_local_manifest`，
`Store::install_external_snapshot` 复用现有 `reopen` + `install_replay_state`，
`Request::ReopenExternal` 作为 shard 入口（仅 Cloud/StandbyReplica 模式）。

拆分：`issue/todo/sto-64a-cloud-adopt-remote-manifest.md`。

## DONE（2026-11-04）

可执行部分已由 `sto-64a-cloud-adopt-remote-manifest.md` 落地；tag/archive 归 12a。

## 复审（2026-11-05 r6）

**通过（范围收缩后由 sto-64a 闭合）。** 核实：真缺口两点（Cloud 本地旧 manifest 命中不回源、缺「只删本地」原语）均已由 sto-64a 落地（本次复审通过）；「reopen_on_missing 挂 waiter」由 eng-39c（done）先行修复的前提在 64a 复核中一并确认（`fkv_shard/tests/cloud_reopen.rs:106` 族）。tag/archive 归 12a（todo）。
