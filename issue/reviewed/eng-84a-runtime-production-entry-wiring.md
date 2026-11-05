# eng-84a P0：Runtime 生产入口接线（facade / SDK / CLI）

来源：`issue/todo/eng-84-global-api-unwired-and-list-archive-tags-missing.md`
的「入口决策包」选项 A（facade 暴露库表面）+ 选项 B 的最小 CLI 形态。
本文件只承接「Runtime 生产入口」这一 P0；eng-84 里的
`global_list_archive_tags` 语义与「空目标集合 result name」残余仍留在 eng-84。

## 决策

C++ eloqstore 是**库**（`include/eloq_store.h` 的 `HandleGlobal*` 由嵌入方调用），
不是自带网络服务的进程。Rust 对等物因此是 facade 暴露的库表面，不新造 server。

## 改动

### `fkv_runtime`

- `impl Default for RuntimeConfig`：默认值与 `KvOption::default()` 对齐，
  `start_with_option` 的 `merge_field` 不会把「未显式指定」误判成两侧冲突；
  `large_value_threshold = 0` 交由 shard 按 `page_size / 2` 自动取界。
- `RuntimeConfig::new(root)`：只填数据根，其余取默认。
- `Runtime::start_mem(config)`：`Backend::Mem` 的唯一对外入口。`StoreMode`
  没有 Mem（与 C++ `StoreMode` 一致），因此不做 `KvOption` 字段；
  Mem 下 `root_li` 置空，全局列举 / 定时归档不扫本地盘。
- `start_impl` 增加内部 `backend_override`；`start` / `start_with_option`
  行为不变（override 为 `None`）。

### `fkv` facade

- 依赖 `fkv_runtime` / `fkv_api`。
- 重导出 `Runtime`、`RuntimeConfig`、`ArchiveEntry`、`GlobalReply`、
  `PrewarmReport`、`Request`、`Response`、`StoreMode`、`BatchEntry`、
  `MAIN_BRANCH_NAME` 及 `Request`/`Response` 的载荷类型。
- 新增 `start_runtime(root, &KvOption)`：Local / Cloud / Standby 全部由
  `KvOption::derive_mode()` 决定；`start_mem_runtime(root)`：纯内存。

### `fkv_sdk`

- `pub use fkv_btree::BatchEntry`（`put` 的公开签名本来就要求它）。
- crate 文档写清 Local-only 与 facade 入口；非 Local 配置仍显式 `InvalidArgs`。

### `fkv_cli`

- 依赖改为 facade（`fkv`），不再直接依赖 `fkv_sdk` / `fkv_config` /
  `fkv_error` / `fkv_types`。
- 新增全局 `--mode auto|local|cloud|standby|standby-master|standby-replica|mem`
  与 `--term`（默认 1，0 拒绝）；所有配置仍来自 `KvOption`（`--config <ini>`）。
- `--mode auto`（默认）按 `KvOption::derive_mode()` 分流：Local 走 `KvStore`
  便捷路径（保持原行为），Cloud / Standby 走 `Runtime`。显式模式与配置派生
  模式不一致时返回 `InvalidArgs`，不静默清空 cloud / standby 配置。
- Cloud / Standby 的 Runtime 路径先 attach 全部 shard，再执行命令，最后
  `shutdown`；数据命令不显式 `reopen`（Local/Mem 懒创建空表，
  Cloud/StandbyReplica 由 shard 的 `auto_reopen` 等 manifest 可见），
  避免一次 CLI 调用空等 `auto_reopen_retry_times × auto_reopen_pending_time`。
- 新增控制面子命令：`archive` / `list-archive-tags` / `reopen-archive` /
  `create-branch` / `drop-table`，把 4+1 个 `global_*` 接进生产入口
  （Local 模式下控制命令也会走 Runtime）。

### `tool/orphan_allow.tsv`

- 删除 `fkv_runtime` 行。
- 接线后 `fkv_cloud` / `fkv_task_prewarm` / `fkv_object_s3` / `fkv_io_mem`
  同属从顶层交付物可达的生产依赖，白名单行一并删除（否则守卫报
  「白名单已过期」）。保留 `fkv_io_sim`（test-only，无生产消费者）。

## 测试

- `crate/facade/fkv/tests/runtime_entry.rs`（4 条）：
  Local 经 `Request`/`Response` 收发；StandbyMaster 重启后 `reopen` 读回；
  Mem put/get 且不落盘；Cloud 按 `KvOption` 装配启动。
- `crate/engine/fkv_runtime/tests/modes.rs`：新增 Mem put/get + 全局列举为空 +
  不落盘。
- `crate/facade/fkv_cli/tests/cli.rs`：Standby 模式跨进程 put/get +
  `archive` / `list-archive-tags` / `reopen-archive`；Mem 进程内有效、跨进程
  读不到；`--mode cloud` 缺 `cloud_store_path` 失败；未知模式失败。
- 命令与结果：
  - `cargo nextest run -p fkv -p fkv_cli -p fkv_sdk`：17 passed。
  - `cargo nextest run -p fkv_runtime`：61 passed / 1 skipped。
  - `cargo clippy -p fkv -p fkv_sdk -p fkv_cli -p fkv_runtime --all-targets
    --no-deps -- -D warnings`：EXIT=0。
  - `bash tool/orphan.sh`：EXIT=0（白名单 1 项：`fkv_io_sim` test-only）。

## 已知边界

- Cloud 真实 put/get 需要外部 S3（沿用 `EKV_S3_ENDPOINT` 门槛）；离线测试覆盖
  Cloud 装配启动，Local/Standby/Mem 覆盖真实读写。
- `Runtime::start_mem` 无 durability；Mem 下 `global_archive` 等控制面目标集合
  为空（无盘可归档）。
- CLI 的 Local 便捷路径（`KvStore`）不写 term 文件；控制面命令改用 Runtime
  attach term 1 后，同目录数据仍可读写（data 文件按 file id 解析、manifest
  跨 term 可读），但 SDK 路径不参与 term fencing —— 属既有 SDK 语义，
  不在本次范围。

## DONE

2026-11-05：facade / CLI 入口接线完成；`tool/orphan_allow.tsv` 的
`fkv_runtime` 行已删除，`bash tool/orphan.sh` EXIT=0。

## 复审（2026-11-05 r7）

**通过。** 逐项核对（fkv_runtime / fkv / fkv_sdk / fkv_cli 在复核时点 03:48–03:49
仍被并行会话触碰，但下述内容全部在位，与声称一致）：

- `fkv_runtime`：`impl Default for RuntimeConfig`（lib.rs:72，`large_value_threshold: 0`
  :86）；`RuntimeConfig::new` :102；`start` :1068 / `start_with_option` :1073 /
  `start_mem` :1085（文档注明 Mem 不落盘、目标集合空）；`start_impl` 带
  `backend_override` :1089-1092，override 生效 :1150，Mem 分支 `root_li` 置空 :1158。
- facade `fkv`：重导出齐（src/lib.rs:20-29，含 `MAIN_BRANCH_NAME` / `StoreMode` /
  `BatchEntry` 等）；`start_runtime` :35-37（`start_with_option(RuntimeConfig::new(root), option)`）、
  `start_mem_runtime` :42-44。
- `fkv_sdk`：`pub use fkv_btree::BatchEntry`（src/lib.rs:15）。
- `fkv_cli`：`--mode` 七值 + `--term` 默认 1、0 拒绝（main.rs:77-84、:390-393）；
  `archive` / `list-archive-tags` / `reopen-archive` / `create-branch` / `drop-table`
  子命令定义 :434-454、分发 :324-353；Cargo.toml 依赖仅 `fkv`（facade），无
  sdk/config/error/types 直依赖。
- `tool/orphan_allow.tsv`：仅剩 `fkv_io_sim	test-only`，头部注释记录五行删除。
- 测试：`fkv/tests/runtime_entry.rs` 4 条（local 收发 :24 / standby 重启 reopen :60 /
  mem put/get 且断言 `demo.1` 不落盘 :92-108 / cloud 装配启动 :111-125）；
  `fkv_runtime/tests/modes.rs` `mem_mode_put_get_without_disk` :136-141；
  `fkv_cli/tests/cli.rs` 6 条（standby 跨进程 put/get + archive/list-archive-tags/
  reopen-archive :58-118、mem 进程内 :121、cloud 缺配置失败 :144、未知模式 :156、
  mem 拒配置 :168）。

残留（已有归口，不在本条）：eng-84（todo）的「空目标集合 `GlobalReply::name`」。
