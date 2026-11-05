# rev-46 P2：StandbyReplica 懒开表不调 sync_standby_partition，可复放过期本地 manifest

来源：rev-34（`issue/reviewed/rev-34-cloud-reopen-local-manifest-stale.md`）DONE 明言
的带出残留：「StandbyReplica 的 `ensure_table_ready` 懒开路径不调用
`sync_standby_partition`（后端是 `CompioStore`，不是 CloudStore），本修复不覆盖」。
r7 复审确认该残留真实在盘且无任何条目跟踪，立项承接。

## 证据（2026-11-05 r7 亲读；fkv_shard/src/lib.rs 并行热改，行号以内容锚点为准）

- `sync_standby_partition`（fkv_shard:1504-1527）只在三个显式 reopen 分发点调用：
  `Request::Reopen`（:2415）、`Request::ReopenArchive`（:2425）、
  `Request::ReopenExternal`（:2438）。
- `ensure_table_ready`（:902-933）main branch 直调 `state.store_mut()?.reopen(tbl)`
  （:910-911），无任何 standby 同步；Standby 后端是本地 `CompioStore`，其
  `read_manifest` 读本地文件，无 Cloud 的回源护栏（rev-34 修的是 `CloudStore`）。
- 触发前提：standby 副本进程重启 / 换主后，某表**首次**经读请求懒开（此前无
  `Request::Reopen` 落到该表）→ 以本地上次 rsync 的旧 manifest replay → 在下一次
  sync 周期前读到过期 root（静默丢读窗口）；若该状态上发生写（standby 提升场景），
  以 stale 基线写回的风险面与 rev-34 描述同型。
- auto-reopen 队列重发的也是 `Request::Reopen`（有 sync 保护），因此窗口限定在
  「懒开先于任何显式 Reopen」的首次访问。

## 修法建议

Standby 分支下 `ensure_table_ready` 的 main-branch 懒开先 `sync_standby_partition`
再 `store.reopen`（其内部 `evict_table` 已处理 rename 替换后的 FD/路径缓存失效）；
注意 sync 失败（master 上无该分区）时应走既有 `ResourceMissing` 挂队列语义，不能
装空表。`reopen_on_missing` 分叉逻辑保持不变。

## 验收（fkv_shard 侧测试）

- 本地预置旧 manifest（旧数据）→ standby 懒开首读即触发 sync，读到 master 当前数据；
- master 无该分区 → 懒开不装空表，进入重试队列（对齐 eng-39c 语义）。

## 甄别（2026-11-05 r7 立项）

- 去重：todo/rev-37 是 auto-reopen 重试策略；todo/rev-38 是 reopen 后无预热
  （性能）；全 issue/ grep `sync_standby_partition|懒开` 仅命中本条出处 rev-34。
- 与 rev-34 的边界：rev-34 已修 CloudStore 回源（reviewed）；本条是 StandbyReplica
  本地后端的同族缺口，rev-34 DONE 明确划出、无执行载体，非重复。
- P2：需要「重启 / 换主 + 懒开先于显式 Reopen + 下次 sync 前读」三重时序，
  窗口有限但生产路径可达，且 standby 语义是正确性敏感面。

## 甄别（2026-11-05 r7）

**结论：采纳（P2 维持）。** Rust 事实全部在盘复核属实；两处修正（后果更宽、
C++ 对照结论相反），已并入下方开发计划。

### 证据复核（亲读，2026-11-05 盘面）

- `sync_standby_partition`（fkv_shard/src/lib.rs:1504-1527）全仓生产调用点仅
  三处：`Request::Reopen`（:2415）、`ReopenArchive`（:2425）、`ReopenExternal`
  （:2438）——与立项一致。`ensure_table_ready`（:902-933）main 分支直调
  `store_mut()?.reopen(tbl)`（:911），经 :1486（列举类准入）与 :2131（读准入）
  两条懒开入口均无 standby 同步。standby 后端确为本地 `CompioStore`
  （fkv_shard/src/backend.rs:24-25/:65），manifest 读取无回源护栏。
- `sync_table_with_term` 在 fkv 全仓生产代码唯一调用点是 fkv_shard:1518
  （其余全在 fkv_standby 自身测试）；fkv_runtime 无周期 catch-up 循环
  （仅 archive crond）。**修正一（后果）**：立项称「在下一次 sync 周期前读到
  过期 root」——进程内根本不存在 sync 周期，追赶只挂在 reopen 路径
  （fkv_runtime/src/lib.rs:936/:976 注释明言）；stale 状态持续到控制面发来
  显式 `reopen()`/`global_reopen()`（fkv_runtime:1462/:1860）为止，窗口由
  控制面行为决定，**比「首开时机」更持久、比「下一周期」更宽**。P2 定级因此
  成立而非降级。

### C++ 对照（修正二：这不是 Rust 转写遗漏，是两边共有的设计性质）

- C++ `ReopenTask::Reopen`（eloqstore/src/tasks/reopen_task.cpp:26-59）确实
  StandbyReplica 下先 `RsyncPartition` + `WaitIo`——Rust 的三个显式入口已对齐。
- 但 C++ **懒开路径同样不 rsync**：read_task.cpp:38/:201 → `PageManager::FindRoot`
  （storage/page_manager.cpp:162-209）→ `IoMgr()->GetManifest`；standby 的
  IoMgr 是 `IouringMgr::GetManifest`（async_io_manager.cpp:933-969），纯本地
  FD+statx 读，无任何远端动作。`RsyncPartition/CatchUp/SyncTable` 在 C++ 全树
  （src/include/tests）唯一生产调用点就是 reopen_task.cpp:41。且
  eloq_store.h `AttachShard` 文档明文设计「Each partition then takes over
  lazily, on its first open」——C++ 把懒开新鲜度交给控制面的 Reopen。
- 因此：Rust 现状**忠实于 C++**；本条修法（懒开先 sync）是**有意超越 C++ 基线
  的正确性改进**，不是对齐修复。必须登记为刻意差异，否则后续 compat 对齐轮
  可能以「C++ 也这样」为由回退。

### 去重复核

- rev-34 DONE 残留段（reviewed/rev-34:216-218）明文划出本条且无执行载体；
  12b/12d（standby wiring）只验证 Reopen 路径接通；cld-58（多根 master，已修）、
  cld-44/45/47、sto-70、todo/rev-37（重试轮数）、rev-38（预热）均正交。无重复。

### 开发计划

1. `ensure_table_ready` main 分支 standby 惰况（`state.standby.is_some()`）先
   `sync_standby_partition` 再 `store.reopen`；其内部 :1514 `evict_table` 已处理
   rename 后 FD 失效。sync 失败 `NotFound/ResourceMissing` → 沿既有挂队列语义
   （:2133 读准入已具备），不得装空表；`reopen_on_missing` 分叉不动。
2. 测试按本文件验收段两条（旧 manifest 首读追平；master 无分区入重试队列）。
3. **登记刻意差异**：`COMPAT_BASELINE` + `doc/architecture/`（persistence 分册）
   记「fkv standby 懒开先追 master vs C++ FindRoot 懒 replay 本地 manifest」，
   理由：静默丢读窗口在副本重启/换主后无进程内机制收敛。

### 热点文件标注

- `crate/engine/fkv_shard/src/lib.rs`：并行会话热改（本次复核 mtime 当日
  03:48，backend.rs/roots.rs 03:59 仍在动）；行号以符号
  `ensure_table_ready`/`sync_standby_partition`/`process_envelope` 定位。
- `crate/io/fkv_standby/src/lib.rs`、`crate/engine/fkv_runtime/src/lib.rs`：
  接手前先 `git status` 确认无并行会话占用。

## DONE（2026-11-05）

**状态：代码 + 测试闭环，issue 关闭。**

### 改动（`crate/engine/fkv_shard/src/lib.rs`）

`ensure_table_ready` 的 main branch 懒开路径：`state.standby.is_some()` 时先
`sync_standby_partition(state, tbl)` 再 `ensure_store` / `store.reopen`。

- 覆盖三条懒开入口（读准入 `submit_read_li`、`prepare_table`、列举类准入），
  StandbyReplica 本地 `CompioStore` 不再 replay 上次 rsync 的旧 manifest。
- `sync` 失败 `NotFound/ResourceMissing` 时 `sync_standby_partition` 已
  `drop_manifest` 并原样返回 `ResourceMissing`，上层沿用 Cloud/Standby 的
  park → driver 重试语义；不会 `open_empty` 装空表。
- `reopen_on_missing` 分叉未改；branch 模式与显式 `Reopen` 路径保持原样
  （后者本就在 `dispatch` 里 sync）。
- 代码注释登记：这是**有意超越 C++ 基线**的改动（C++ 懒开 `FindRoot` 也 replay
  本地 manifest），理由是副本重启 / 换主后进程内无其它收敛机制。

### 测试（`fkv_runtime/tests/standby.rs`，需 rsync，缺失时 skip）

- `replica_lazy_open_syncs_master_before_replaying_local_manifest`：master 已有
  数据，replica 不调用 `reopen` 首次 `get` 即读到数据，本地文件与 master 逐字节一致。
- `replica_lazy_open_after_restart_reads_current_master`：副本同步 `old` 后停机 →
  master 更新为 `new` → 新副本进程（同一盘面、本地是旧 manifest）懒开首读拿到
  `new`，覆盖「重启 + 懒开先于显式 Reopen」窗口。
- `replica_lazy_open_missing_master_does_not_create_empty_table`：master 无分区时
  懒开回 `ResourceMissing`，本地不创建 `table_dir`（不装空表）。

### 残留（受本次 lane 文件范围限制）

- `COMPAT_BASELINE` / `doc/architecture/05-persistence-and-recovery.md` 的
  「fkv standby 懒开先追 master vs C++ 懒 replay」刻意差异登记未做
  （只允许改 fkv_shard / fkv_runtime / fkv_task_reopen / fkv_standby 及测试）；
  代码注释已就地说明，后续文档维护者需补登记。

## 复审（2026-11-05 v）

代码 + 测试通过，归档；**登记残余转 [`v-09`](../new/v-09-rev46-standby-lazy-open-divergence-unregistered.md)**。

- 代码属实：`fkv_shard/src/lib.rs:1068-1070` `if branch == MAIN_BRANCH_NAME &&
  state.standby.is_some() { sync_standby_partition(state, tbl).await?; }`
  前置于 `store.reopen` :1075；`sync_standby_partition` :1872；注释 :1062-1067
  写明「有意超越 C++ 基线」（C++ `FindRoot` 纯本地 replay）。
- 测试属实：`fkv_runtime/tests/standby.rs:284`
  `replica_lazy_open_syncs_master_before_replaying_local_manifest`、:322
  `replica_lazy_open_after_restart_reads_current_master`、:388
  `replica_lazy_open_missing_master_does_not_create_empty_table`。
- 特别核对（刻意差异登记）：**未落**——`COMPAT_BASELINE` 全文无 rev-46 条目
  （`grep standby|懒开|lazy|FindRoot` 零命中）；`grep -rn 懒开 doc/` 零命中；
  仅 `doc/architecture/06-gaps.md:83` 作能力陈述。DONE「残留」段自认且无归口
  issue，已拆 v-09 承接（同 r8c-02↔rev-37 模式）。
