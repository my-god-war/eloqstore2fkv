# rev-34 Cloud 模式 reopen 本地 manifest 优先，接管后读到过期快照并以 stale 基线覆盖远端

## 位置
- Rust: `crate/engine/fkv_cloud/src/lib.rs:1770-1780`（`read_manifest` 本地优先）、`crate/engine/fkv_store/src/lib.rs:1654-1677`（`Store::reopen` main branch 走 `reopen_with_size`）、`crate/engine/fkv_shard/src/lib.rs:1601-1609`（`Request::Reopen` dispatch 不调 `refresh_manifest`）
- C++: `src/tasks/reopen_task.cpp:100-107`（`InstallExternalSnapshot` 读远端）、`src/async_io_manager.cpp:2982-2983`（Cloud 本地 manifest 读远端快照）

## 现状
Cloud 后端 `read_manifest`（fkv_cloud）先读本地 `CompioStore` 的 manifest，**命中即返回，
不比较远端 term / 不刷新**：

```rust
// fkv_cloud/src/lib.rs:1770-1773
async fn read_manifest(&self, tbl: &TableIdent) -> Result<Option<Vec<u8>>> {
  if let Some(bytes) = self.local.read_manifest(tbl).await? {
    return Ok(Some(bytes));          // 本地命中即返回，不回源
  }
  ... latest_remote_manifest_bytes ...  // 仅本地缺失才回源
}
```

而本地 manifest 文件名与 term 无关：`CompioStore::manifest_path` 恒为
`manifest_<MAIN_BRANCH_NAME>_0`（`fkv_io_compio/src/lib.rs:1882-1886`，main branch 写死
term 0），**不会随 attach 的新 term 变化**。`Request::Reopen` 走 `Store::reopen →
reopen_with_size → io.read_manifest`（main branch 分支，`manifest_file == None`），
在 Cloud 模式下**不经过 `refresh_manifest` / `install_external_snapshot`**
（只有 `Request::ReopenExternal` 才 `refresh_manifest`，`fkv_shard/src/lib.rs:1610-1630`）。

`prepare_attach`（`fkv_store/src/lib.rs:1587-1602`）只做 `reset_tables`（清内存表 +
`invalidate_path_cache`），**不删本地 manifest 文件**。Cloud `evict_table`
（`fkv_cloud/src/lib.rs:1682-1687`）同样只 `forget_table_cache` + 清 file_range_map，
不删本地 manifest。

## C++ 实现
Cloud 下 reopen 的语义是"采纳远端当前快照"。`ReopenTask::Reopen`（standby/cloud 路径）
经 `IndexManager::InstallExternalSnapshot` 读**远端** manifest 安装；Cloud 本地文件只是
cache，远端是权威。跨 term 接管时，进程带 `expect_term` 打开 mapper，`Replayer::GetMapper`
（`src/replayer.cpp:350-368`）在 `manifest_term != expect_term` 时把 allocator 抬到
**下一 file 边界**，保证新 term 不与旧 term 复用 file id 空间；读路径也总以远端 manifest
定位最新页集合。本地 cache 里的旧 manifest 在 Cloud reopen 中不作为可信来源。

## 差异与风险
Cloud 模式 shard 以 term=T2 接管一个之前由 term=T1 写过的分区（同 branch），且此前进程
已在本地落过 `manifest_MAIN_0`（或 detach 未清干净）：

1. `Shard::Reopen` / 首次 `ensure_table_ready` 命中本地 stale manifest，replay 出
   **过期 root**，`is_open` 置真，后续 `Get`/`Scan` 读到旧数据，看不到 T1→T2 之间已提交到
   远端的写入（静默丢读）。
2. 更危险：随后在该分区上 `Put` 会以 stale root 为基线做 CoW，分配 file page（虽然
   `install_replay_state` 在 term 变化时 `next_file_boundary` 抬高了 `max_fp_id`，但
   mapping 仍来自 stale 快照），append manifest 时 `put_manifest_with_retry` 把本地
   manifest **以当前 identity 覆盖写回远端**（`manifest_key` 按当前 branch+term），可能
   覆盖更高 term 写下的远端权威 manifest，造成丢写 / 分叉。

`install_replay_state` 的 term-bump 只保护 file id 复用，不校验"本地 manifest 是否
落后于远端当前版本"。这是 Cloud 权威性问题：本地 cache 不能对 reopen 谎报"我有更新的
manifest"。

## 对齐建议
Cloud / StandbyReplica 模式下，让 reopen 的 main-branch 路径也先 `refresh_manifest`
（强制回源比对远端 term 与内容，取远端为准）再 replay，复用 `install_external_snapshot`
的语义；或至少让 Cloud 本地 manifest 文件名带当前 term（`manifest_<branch>_<term>`），
term 变化后本地自然 miss 从而回源。`prepare_attach` 在 Cloud 模式应 `drop_local_manifest`
（对齐 C++ detach/attach 的 cache 失效），避免残留旧 term 的本地 manifest 被后续 reopen
优先命中。

## 严重度
P1

## 核验记录
- `fkv_cloud/src/lib.rs`：`read_manifest`（1770-1780，本地命中直接返回）、
  `refresh_manifest`（1809+，仅 ReopenExternal 用）、`evict_table`（1682-1687，不删
  manifest）、`manifest_key`（598-601，写按 identity）。
- `fkv_io_compio/src/lib.rs:1882-1886`：`manifest_path` 硬编码 `manifest_file_name(MAIN, 0)`。
- `fkv_store/src/lib.rs`：`reopen`（1654-1677，main branch 走 `reopen_with_size` 无
  refresh）、`prepare_attach`（1587-1602）、`install_external_snapshot`（1681-1727，
  仅 `ReopenExternal` 调用）、`install_replay_state` term-bump（1712-1730）。
- `fkv_shard/src/lib.rs`：`Request::Reopen`（1601-1609）与 `ReopenExternal`
  （1610-1630）分叉；`ensure_table_ready`（669-691）main branch 走 `store.reopen`。
- C++ `reopen_task.cpp:100-107`（`InstallExternalSnapshot`）、`replayer.cpp:350-368`
  （`GetMapper` 跨 term 抬 file 边界）。
- 说明：本地优先是否触发依赖 stale manifest 存在；本 issue 记录 Cloud reopen 不以远端为
  权威这一确定性对齐差异，触发前提在 Cloud detach 未清本地时成立。

## 甄别（2026-11-05 r6）

**判定：采纳（P1 维持）。** 逐条复核 Rust / C++ 证据全部属实（非半途伪差异），且与
sto-64a 等已完成工作不重叠：sto-64a 修的是「**没有**采纳远端的入口」，本条说的是
「**默认** reopen 入口仍本地优先」这一残留分歧。

### 证据核实（行号为 2026-11-05 实测；fkv_cloud/fkv_store/fkv_shard 并行热改中，
原文行号已漂移，实现时按内容锚点定位）

- `crate/engine/fkv_cloud/src/lib.rs:1890-1900` `CloudStore::read_manifest`：本地命中
  即 `return Ok(Some(bytes))`，不比对远端 term、不回源；仅本地 miss 才走
  `latest_remote_manifest_bytes`（:2100-2116，远端同 branch 取最大 term）。
- `crate/engine/fkv_cloud/src/lib.rs:1802-1807` `evict_table`：只
  `forget_table_cache` + 清 `file_range_map`，不删本地 manifest；`:1944-1946`
  `drop_local_manifest` 存在但 reopen 路径无人调用。
- `crate/io/fkv_io_compio/src/lib.rs:1878-1882` `manifest_path` 恒为
  `manifest_MAIN_0`（term 盲），跨 term 本地命中可能。
- `crate/engine/fkv_store/src/lib.rs:1738-1757` `Store::reopen`：main branch 走
  `reopen_with_size(&self.io, tbl)` → `io.read_manifest`（Cloud 即上述本地优先），
  无 refresh。`:1613-1628` `prepare_attach` 只 `reset_tables`（清内存 +
  `invalidate_path_cache`），不删本地 manifest；`:3411-3414` `claim_term` 只
  `upsert_term`。`:1788+` `install_replay_state` 的 term-bump 只抬 allocator，
  不校验本地基线是否落后远端（与意见描述一致）。
- `crate/engine/fkv_shard/src/lib.rs:2272-2281` `Request::Reopen`：Cloud 下
  `sync_standby_partition` 为 no-op（:1417-1420，仅 Standby 分支做事），随后直接
  `store.reopen`；`:2282-2307` 仅 `Request::ReopenExternal` 走
  `install_external_snapshot` → `refresh_manifest`。`:877-906`
  `ensure_table_ready` main branch 懒开同样直调 `store.reopen`（无回源）；
  `ReopenQueue` 到期重试重发的也是 `Request::Reopen`（:1036）。`:2382-2418`
  `AttachShard` = `claim_term` + `prepare_attach`，`:2419-2439` `DetachShard`
  只对 open 表 `evict_table` —— 本地 manifest 文件跨 detach/attach 全程保留。
- 写回晋升链成立：`append_manifest`（fkv_cloud:1868-1879）本地追加后按
  `manifest_key`（:702-705，`manifest_<branch>_<当前term>`）上传；cld-56 之后
  远端选择取最大 term → 以 stale 基线在更高 term 写回的 manifest 会成为权威，
  遮蔽先前 term 的已提交数据。
- C++：`reopen_task.cpp:14-22`（Local → InvalidArgs，Cloud 直达 install）、
  `:63-97`（clear_local_state → `DropLocalManifest` 只删本地）、`:100-107`
  （`InstallExternalSnapshot`）；`page_manager.cpp:530-545`（Cloud →
  `CloudStoreMgr::RefreshManifest` 无条件回源）、`:546-584`（NotFound →
  DropLocalManifest + 清内存，绝不删远端）。**即 C++ Cloud 模式普通 Reopen 的
  语义就是采纳远端**，Rust 默认路径与之分歧属实。

### 勘误（不影响结论）

- C++ 引用 `async_io_manager.cpp:2982-2983` 标注「读远端快照」不准确：该处是
  `SwitchManifest` **写路径**，证明的是 C++ 本地 manifest 文件名带 `ProcessTerm()`
  （跨 term 本地自然 miss）；「读远端」实际在 `page_manager.cpp:532-538` /
  `CloudStoreMgr::RefreshManifest`。实质成立，引用标签需更正。
- 「detach 未清干净」措辞偏宽：detach / attach / evict / claim_term **从不**清理
  本地 manifest（非偶然残留），任何本地目录复用即满足触发前提。
- 同一 term 内本地与远端由 `append_manifest`（本地追加后即上传）保持同步，stale
  仅出现在跨 term 复用本地目录（被 fencing 后重新以更高 term 接管 / 重启复用
  cache 目录）——正是意见所述接管场景，生产路径可达。
- Standby 侧 nuance：plain `Request::Reopen` 有 `sync_standby_partition`（rsync
  替换本地 manifest）保护，但 `ensure_table_ready` 懒开路径 Standby 同样直调
  `store.reopen` 不 sync，并未完全豁免；开发计划一并覆盖。

### 与相邻 issue 的边界

- **sto-64 / sto-64a（reviewed，done）**：其审查已点名「Cloud 本地 cache 有旧
  manifest 时永不回源」，但处置是**新增** `ReopenExternal` 入口（最小闭环），
  plain `Reopen` / `ensure_table_ready` / auto-reopen 重试仍本地优先 —— 本条即该
  残留，未被 64a 覆盖。
- **cld-56（reviewed）**：修的是**回源时**选哪个远端 manifest（同 branch 最大
  term）；本条是**根本不回源**。cld-56 的 `cross_term_reopen` 测试本地 cache 为空，
  不触发本地命中分支。
- **eng-39c（reviewed）**：修 `ResourceMissing`（manifest 缺失）不得 open_empty、
  挂队列重试；本条是 manifest **存在但过期**，正交。
- **eng-39a/b、rev-37（new）**：driver/waiter 并发、死代码、重试策略参数，均不触
  及权威性。**rev-38（new）**：reopen 后无 prewarm，性能面。
- **core-22b（done）**：data 文件按 (branch,term) 寻址；**sto-70（reviewed）**：
  standby 安装落本地活跃名 `manifest_main_0` + 清旧 term 文件 —— 后者反而佐证
  本地活跃名是 term 盲的固定名，是本条前提。

### 开发计划

1. 主修（语义对齐 C++）：`reopen_on_missing`（Cloud/StandbyReplica）为真时，
   main-branch reopen 三入口统一改走 `Store::install_external_snapshot`（refresh
   回源 → reopen）：`ensure_table_ready`、`Request::Reopen` dispatch、`ReopenQueue`
   重试（重发 `Request::Reopen` 自动继承）。Local/Mem 保持现状
   （`refresh_manifest` 默认 no-op，且不能引入 `clear_local_snapshot` 的删本地
   语义），按模式分叉而非全局替换。
2. 纵深（可选）：`append_manifest` / `switch_manifest` 写回前比对远端同 branch
   最大 term（复用 `latest_remote_manifest_bytes` 的列举），远端更高则拒绝
   （`ExpiredTerm`）或先 refresh，防 stale 基线晋升权威。
3. 不采纳「本地 manifest 文件名带 term」：与 sto-70 固定本地活跃名
   `manifest_main_0` 的 standby 安装对齐冲突，牵动 compio 命名与 rsync 面。
   `prepare_attach` 在 Cloud 下 `drop_local_manifest` 作为备选（次优：同 term
   合法 cache 也丢）。
4. 测试（`fkv_shard/tests/cloud_reopen.rs`）：(a) 本地预置 stale manifest（旧
   root/旧数据）+ 远端更高 term 新 manifest → 懒开 / `Reopen` 后 `Get` 读到远端
   新数据；(b) 该状态上 `Put` 后，更高 term 新实例 reopen 仍能看到此前远端提交
   （写回不晋升 stale 基线）。反向断言：不修则 FAIL。

### 热点文件

`crate/engine/fkv_cloud/src/lib.rs`、`crate/engine/fkv_store/src/lib.rs`、
`crate/engine/fkv_shard/src/lib.rs` 均在并行会话热改中（本条原文行号已漂移
~80-670 行不等），实现前先按内容锚点重定位。

## DONE（2026-11-05，Cloud 路径）

改动（`crate/engine/fkv_cloud/src/lib.rs::read_manifest`）：

- **本地命中也不再直接返回**：先取本地 cache，再调用 `latest_remote_manifest_bytes`
  回源（现有逻辑已按同 branch 取最大 term）。
- 本地字节 == 远端当前快照 → 返回本地（同 term 合法 cache，零改写）。
- 本地缺失 / 落后 → `local.switch_manifest` 覆盖本地 cache 后返回远端字节。
- 远端无 manifest、本地有 → `local.drop_manifest` 并返回 `None`
  （对齐 C++ `CloudStoreMgr::RefreshManifest` NotFound → `DropLocalManifest`），
  让 `Store::reopen` 返回 `ResourceMissing` 进入 shard 的重试队列，绝不以 stale
  快照懒创建。
- 用「内容是否相同」而不是解析 manifest 里 term：本地活跃 manifest 文件名 term 盲
  （恒 `manifest_main_0`），而 `append_manifest` / `switch_manifest` 上传的就是本地
  完整字节，等值即「cache 与远端一致」；不等值则远端权威（与 C++ Cloud reopen
  无条件采纳远端一致）。
- 覆盖入口：`Store::reopen`（`Request::Reopen` / `ensure_table_ready` 懒开 /
  auto-reopen 重试）与 `install_external_snapshot` → `refresh_manifest` 均走
  `ManifestIo::read_manifest` 或同源回源，无需改 `fkv_shard`。

测试：`crate/engine/fkv_cloud/tests/stale_manifest.rs`（新增）

1. `stale_local_manifest_is_replaced_by_remote_on_reopen`：
   term 1 提交 `old` → term 2 回源后提交 `new`（远端 `manifest_main_2`）→ 用 term 1
   的本地 manifest 字节预置 term 3 实例的本地 cache → `reopen` 后 `old` / `new`
   都可读；随后以采纳后的基线写 `third`，term 4 实例 reopen 仍能看到三者
   （stale 基线不会写回晋升）。
2. `stale_local_manifest_without_remote_is_not_trusted`：删除远端 manifest、仅留
   本地 stale cache → `reopen` 返回 `ResourceMissing`；远端 manifest 恢复后下一次
   `reopen` 成功且数据可读。
3. 负向验证：临时恢复旧「本地命中即返回」实现，两个测试均失败；恢复修复后通过。

残留（不在本条 Cloud 范围内）：StandbyReplica 的 `ensure_table_ready` 懒开路径
不调用 `sync_standby_partition`（后端是 `CompioStore`，不是 CloudStore），本修复
不覆盖；该 nuance 仍记录在本 issue 的甄别段，需要 shard/standby 侧单独处理。

## 复审（2026-11-05 r7）

**通过（Cloud 范围如声称完成）。** 实现路径与 DONE 一致——未按原开发计划把三
入口改走 `install_external_snapshot`，而是下沉到 `ManifestIo::read_manifest` 统一
回源，覆盖面更广（plain Reopen / 懒开 / auto-reopen 重试全继承），语义等效于
C++ Cloud reopen 无条件采纳远端：

- `CloudStore::read_manifest`（fkv_cloud/src/lib.rs:1946-1970）：本地命中也回源
  `latest_remote_manifest_bytes`（:1952）；字节相等返回本地（:1955）；落后/缺失
  `switch_manifest` 覆写本地后返回远端（:1957-1960）；远端无、本地有 →
  `drop_manifest` + 返回 `None`（:1964-1967，对齐 C++ NotFound → DropLocalManifest，
  让上层走 ResourceMissing 重试队列）。
- 远端 term 护栏：`latest_remote_manifest_bytes`（:2170-2186）按同 branch 取
  **max term** 无 tag manifest（`max_by_key(file_term)` :2180）。
- 入口覆盖：`Store::reopen` main 分支（fkv_store/src/lib.rs:1765）→
  `reopen_with_size`（fkv_task_reopen/src/lib.rs:79-84）→ `io.read_manifest`；
  `Request::Reopen` 分发（fkv_shard/src/lib.rs:2412-2415）与 `ensure_table_ready`
  懒开（:910-911）均经此；`install_external_snapshot`（fkv_store:1782-1783）→
  `refresh_manifest`（fkv_cloud:1999-2011，本地命中也刷新）供 `ReopenExternal`
  （fkv_shard:2438-2448）。
- 测试 `tests/stale_manifest.rs`：`stale_local_manifest_is_replaced_by_remote_on_reopen`
  （term1 提交 old → term2 回源提交 new → term3 实例预置 term1 stale 本地 → reopen
  读到 old+new :88-98 → 写 third 后 term4 reopen 三键全在 :100-120，即写回不晋升
  stale 基线）；`stale_local_manifest_without_remote_is_not_trusted`（删远端仅留
  stale 本地 → `ResourceMissing` :153-157，远端恢复后 reopen 成功 :159-170）。
  负向验证（恢复旧实现两测均败）为一次性人工步骤，不可复查，但两测的断言在
  「本地命中即返回」旧语义下必然失败（第三实例会丢 new / 本地 cache 会起跑），
  结构上自证。
- 残留承接：StandbyReplica 懒开无 sync 的 nuance 已核实仍在盘
  （`ensure_table_ready` 直调 reopen，sync 仅 :2415/:2425/:2438 三处），
  立项 `issue/new/rev-46-standby-lazy-open-skips-sync-standby-partition.md`。
