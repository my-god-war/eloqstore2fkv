# 12c global fanout 与 Cloud 模式收尾

## 来源
`issue/done/12-runtime-modes-api.md` 阶段 4，以及阶段 2 落地后留下的 Cloud 配置缺口。

## 位置

- `crate/engine/fkv_runtime/src/lib.rs`：`ArchiveCrond::tick`（现成的 fanout 范式）、
  `Runtime::{drop_table, create_archive, ...}`、`backend_of`。
- `crate/engine/fkv_shard/src/backend.rs`：`CloudBackend`（`cache_budget` 固定 0）。
- `crate/foundation/fkv_config/src/lib.rs`：`max_global_request_batch`、`prewarm_cloud_cache`、
  `store_path` / `store_path_weights` / `cloud_provider` / `local_space_limit`。
- `crate/engine/fkv_cloud/src/lib.rs`：`cache_budget`、`preload_paths`、远端列举。

## 缺口

1. 无 `GlobalArchive` / `GlobalReopen` / `GlobalCreateBranch` / `GlobalDropTable`；
   `DropTable` 只有 per-table 语义。
2. global fanout 未按 `max_global_request_batch` 分批；`ArchiveCrond::tick` 一次全投。
3. Cloud 模式：`cache_budget` 恒为 0（不限），未映射 `local_space_limit` 等配置；
   `cloud_provider` / `store_path` / `store_path_weights` 未消费。
4. Cloud 模式 archive crond 仍 `scan_table_li(local root)`，只看到本地 cache，需改远端列举。
5. Cloud 模式 attach 后未触发 prewarm（`prewarm_cloud_cache` / `prewarm_task_count`）。
6. 无 Cloud 模式 Runtime 端到端测试（当前只验证装配）。

## 方案

1. 抽出 `GlobalFanout`：
   - 顶层线程 `block_on(join_all(...))`，按 `max_global_request_batch` 分批；禁止在 shard
     poll 栈内驱动调度器（复用 `ArchiveCrond::tick` 写法）。
   - 目标集合来源：Local = 盘上分区目录；Cloud = `ObjectStore::list_page(prefix)` 分页枚举；
     单 shard 失败要聚合上报，不能整体静默。
2. `Runtime` 增加 `global_archive` / `global_reopen` / `global_create_branch` / `global_drop_table`，
   回执含成功数 / 失败 `Vec<(TableIdent, KvError)>`。
3. `backend_of`：`cache_budget` 取 `options.local_space_limit`（0 视为不限），
   `cloud_provider` 非 `aws`/`s3` 时返回 `InvalidArgs`；`store_path` 多根目录另立 issue，
   不在本条硬做。
4. Cloud archive crond：用 `CloudStore` 的远端列举接口替换 `scan_table_li`。
5. attach 成功后按 `prewarm_cloud_cache` / `prewarm_task_count` 投递 prewarm（依赖 12a 的
   `Request::Prewarm`）。

## 验收

- 多 shard（>=2）下 fanout 覆盖全部已 attach 分区；单 shard 注入 `NotRunning` 时回执可见。
- Cloud 模式 archive crond 能枚举远端已有但本地未 cache 的分区。
- `tests/modes.rs` 增补：设置 `local_space_limit` 后 `CloudBackend.cache_budget` 生效。
- `cargo clippy ... -D warnings` 零警告。

## 重叠

- 归档 tag/保留策略：`issue/todo/eng-59`；本条只做 fanout 与列举来源。
- 跨 shard prewarm：`issue/todo/cld-47a-prewarm-background-trigger-and-cross-shard.md`。
- cache 淘汰：`issue/todo/cld-51b-cache-evict-off-hot-path-background.md`。
- 远端列举分页：`issue/todo/io-29*`。

## DONE（2026-11-04）

第 1/2/3/4/5 项落地（6 部分落地），第 4 项的远端路径与第 3 项的 `store_path`
按拆分另立。

要点
- **`GlobalFanout`（第 2 项）**：按 `TableIdent::shard_index` 路由 + 按
  `max_global_request_batch` 分批（每批 `join_all` 顶层并发、批间串行），
  回执 `GlobalReply { ok_num, failed_li: Vec<(TableIdent, KvError)> }`：
  单表失败（含未 attach shard 的 `NotRunning`）逐条可见，不整轮静默。
- **`GlobalArchive` / `GlobalReopen` / `GlobalCreateBranch` / `GlobalDropTable`
  （第 1 项）**：`Runtime::global_archive/global_reopen/global_create_branch/
  global_drop_table`；`Drop` 仍逐分区（只有 per-table 语义，见 sto-62c）。
- **归档 crond 改用 fanout（第 2 项）**：`ArchiveCrond::tick` 从「一次全投」
  改为 `GlobalFanout::dispatch`，并带上 `failed` 计数日志。
- **Cloud 归档列举来源（第 4 项）**：`ArchiveTarget` = `Local(盘上分区目录)` /
  `Cloud(远端 `remote_partition_page` 分页列举)`；Cloud 下 crond 惰性建一个
  运行时侧 `CloudStore`（只做列举、不碰本地 cache，因此不需要 compio runtime）
  并跨 tick 复用。远端列举分页沿用 `cld-47a` 的 `CloudStore::remote_partition_page`。
- **Cloud 配置（第 3 项）**：`backend_of` 的 `cache_budget` 取
  `options.local_space_limit`（`validate` 在 cloud 下把 0 回退为默认 TB）；
  `cloud_provider` 非 `aws`/`s3` 显式返回 `InvalidArgs`（gcs 无实现，不静默按 S3 起）。
- **attach 后预热（第 5 项）**：由 `cld-47a` 落地（同轮同会话）。
- 装配参数抽成 `CloudStoreConfig` + `open_runtime_cloud_store`，预热线程 /
  归档 crond / 全局列举共用同一份投影。

改动文件
- `crate/engine/fkv_runtime/src/lib.rs`：`GlobalReply` / `GlobalFanout` /
  `ArchiveTarget` / `ArchiveState::{target,batch}` / `ArchiveCrond::{tick,enum_target}` /
  `CloudStoreConfig` / `open_runtime_cloud_store` / `backend_of`（provider + budget）/
  `Runtime::{cloud_enum,root,max_global_request_batch}` + `global_*` + 内联测试
  `global_tests`。
- `crate/engine/fkv_runtime/Cargo.toml`：`compio` 依赖（预热线程 runtime）。
- `crate/engine/fkv_cloud/src/lib.rs`：`remote_partition_page`（远端分区分页列举，
  第 4 项的列举接口）。
- 新拆：`issue/todo/12c-followups-store-path-weights-cloud-e2e-and-store-unification.md`。

测试
- `cargo nextest run -p fkv_runtime` → 43 passed / 1 skipped（既有 `long_stress`
  `#[ignore]`）。新增内联：
  - `global_tests::global_drop_table_fans_out_and_reports_not_running`：2 shard、
  批大小 1、盘上 3 分区；attach 仅 shard0 → `total=3 / ok=1 / failed=2(NotRunning)`；
  detach 后 `ok=0`（fanout 不提前退出）。覆盖验收第 1 条。
  - `global_tests::backend_of_maps_cloud_budget_and_rejects_other_providers`：
  `local_space_limit=12345` → `cache_budget=12345`；`gcs` → `InvalidArgs`。
  覆盖验收第 3 条（用内联单测而非 `tests/modes.rs`：`CloudBackend` 不可从
  集成测试访问）。
  - `global_tests::cloud_store_builds_on_plain_thread_and_carries_budget`：
  运行时侧 `CloudStore` 能在非 compio 线程构造且 budget 透传（归档 crond 的
  构造前提）。
- `cargo clippy -p fkv_runtime -p fkv_cloud -p fkv_task_prewarm --all-targets --no-deps -- -D warnings` → 0 警告。

未落地 / 后续（已拆新文件）
- `store_path` / `store_path_weights` 多根目录未接线（本条范围明确外置）。
- `ArchiveTarget::Cloud` 的远端列举 + 归档落盘、Cloud 下 `global_*` 的远端目标集合
  都只有构造级单测，真 S3 端到端测试未落（本地无 S3，`EKV_S3_ENDPOINT` 门控的
  端到端只覆盖了预热）。
- 运行时侧 `CloudStore` 与 shard 实例并行 → `cache_budget` 生效后磁盘上界约
  2× 预算；统一实例需要 `fkv_shard` / `fkv_api`（并发热点）另排期。


## 复审（2026-11-05 r6）

**通过。** 核实：

- `fkv_runtime/src/lib.rs:246` `GlobalFanout`（按 shard_index 路由 + 分批）；:1338/:1353/:1367/:1383 `global_archive/global_create_branch/global_drop_table/global_list_archive_tags`；:76 `ArchiveTarget::{Local,Cloud}`、:383 `enum_target`（Cloud 走远端列举）；:737 `CloudStoreConfig`、:779 `open_runtime_cloud_store`。
- `backend_of`：:881-887 `cloud_provider` 非 aws/s3 → `InvalidArgs`；:901 `cache_budget = local_space_limit`。
- `fkv_cloud/src/lib.rs:2091 remote_partition_page`（远端分区分页列举）。
- 内联测试 `fkv_runtime/src/lib.rs:2076` `backend_of_maps_cloud_budget_and_rejects_other_providers`（:2086 断言 cache_budget=12345、gcs 拒绝）、:2106、:2136 `global_drop_table_fans_out_and_reports_not_running`。
- 未落地项已如实拆分至 12c-followups（done）与 todo/12d，无假完成。
