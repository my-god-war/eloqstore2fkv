# r7c：runtime / shard / SDK / CLI / config 只读审查 findings

范围：`fkv_runtime`、`fkv_shard`、`fkv_api`、`fkv_recovery`、`fkv_task_reopen`、
`fkv_sdk`、`fkv_cli`、`fkv_config`，以及 `doc/architecture/05-persistence-and-recovery.md`
与相关注释的一致性。只读审查，未改任何代码。

结论：**FAIL**（主因 F1：配置含 `store_path` 时 CLI Local 数据命令不可用；F2：S3 并发上界
宣称的整机不变量不成立）。

验证命令（均通过）：

- `cargo check -p fkv_runtime -p fkv_shard -p fkv_sdk -p fkv_cli -p fkv_config`
- `cargo clippy -p fkv_runtime -p fkv_shard -p fkv_sdk -p fkv_cli -p fkv_config
  --all-targets --all-features --no-deps -- -D warnings`
- `cargo nextest run -p fkv_runtime -p fkv_shard -p fkv_sdk -p fkv_cli -p fkv_config`
  → 187 passed / 1 skipped；`-p fkv_sdk -p fkv_cli -p fkv_config` → 41 passed

---

## F1（P0）CLI Local 数据命令对含 `store_path` 的正常配置直接失败

- **file:line**：`crate/facade/fkv_cli/src/main.rs:489-493`、
  `crate/facade/fkv_sdk/src/lib.rs:54-58`
- **现象**：`main` 在 `derive_mode() == Local && !is_runtime_command(name)` 时走
  `run_local` → `KvStore::new(root, option, term)`；SDK 要求
  `store_path.len() == 1 && Path::new(store_path[0]) == root`，否则 `InvalidArgs`。
  `store_path` 是 eloqstore 的 `[permanent]` 核心键，几乎任何生产 ini 都会设置，
  而 CLI 的 `--root` 默认 `.data/fkv`，两者必然不等。
  实测（`target/debug/fkv`）：

  ```text
  # /tmp/fkvcfg/kv.ini: [run] num_threads=1 / [permanent] store_path=/tmp/fkvcfg/data
  fkv --config kv.ini --root /tmp/fkvcfg/root put --table demo.1 --key a --value A
  → Error: InvalidArgs (exit 1)
  fkv --config kv.ini --root /tmp/fkvcfg/root archive --tag t
  → archive ok=0 failed=0 ... (exit 0)          # 同一份配置走 Runtime 成功
  # 去掉 store_path 后 put/get 正常；--root 恰等于 store_path[0] 时也正常；
  # 多根 store_path（/a,/b）同样 InvalidArgs，即使 Runtime 的 RootedStore 支持
  ```

  控制面命令可用、数据面命令不可用，同一配置两套行为，且错误只回 `InvalidArgs`
  无任何提示。
- **修法**：Local 分支不要无条件走 SDK。`store_path` 非空（或根数 > 1）时改用
  `start_runtime`（`RootedStore`，与 `archive` 等命令同一装配），仅在 Runtime
  也无法表达的场景报错；或最小改动：`run_local` 用 `option.store_path[0]` 作为
  SDK 的 root，只在 `store_path.len() > 1` 时给出明确错误信息。
- **优先级**：P0（配置驱动的本地跑法直接不可用）。

## F2（P1）CloudBackend 并发均分未真正约束整机飞行请求数

- **file:line**：`crate/engine/fkv_shard/src/backend.rs:56-85`（`s3_instance_num` /
  `s3_config`）、装配点 `crate/engine/fkv_runtime/src/lib.rs:1240-1246`
- **现象**：注释承诺「按实例数均分，保证整机飞行中请求不超过配置值」，但
  1. 实例数按 `num_threads + 1` 计，实际运行期实例可达 `num_threads + 3`：
     shard ×N、prewarm crond store、archive crond store（`ArchiveTarget::Cloud`）、
     `cloud_enum_store`；并发 `enumerate_target_li` 还会各自临时新建。
  2. `(n / s3_instance_num.max(1)).max(1)` 向下取整后又 `.max(1)`：
     `max_cloud_concurrency < num_threads + 1`（如 20 vs 65）时每实例 1，
     总量 `实例数 × 1 > 20`，直接反超配置值。
  3. `max_cloud_concurrency == 0` 时各实例回落 `FKV_S3_MAX_CONCURRENCY`/默认 64，
     总量 = 实例数 × 64（`validate()` 只在 cloud 模式挡住 0，非 validated 路径仍可达）。
- **修法**：跨实例共享一个 `Arc<Semaphore>`（或原子令牌池）作为整机上限；若坚持
  均分，按真实装配实例数计算并把余数补到前若干实例，同时对
  `max_cloud_concurrency < 实例数` 显式告警或拒绝。
- **优先级**：P1（云端并发/配额超发，故障难定位）。

## F3（P1）root meta 预算检查在每次新建分区时全表 O(n log n)

- **file:line**：`crate/engine/fkv_shard/src/partition.rs:315-354`
  （`enforce_root_meta_budget`），调用点 `:169`
- **现象**：`ensure_store` 每新建一个分区 Store 就调用该函数：遍历全部 slot 累加
  `root_meta_bytes()`、`collect` 候选、`sort_unstable_by_key`。默认
  `root_meta_cache_size = 1 GiB`（runtime 按 shard 均分后仍非 0），因此预算判定恒开启。
  n 个分区冷启动 / 批量 reopen 时总代价 O(n² log n)，十万级分区下首个请求会被
  线性扫描拖慢；`touch` 只是递增代际，无助于降低扫描成本。
- **修法**：增量维护 `root_meta_total`（Store 创建/驱逐/`evict_table` 时加减），
  仅在超预算时做排序裁剪；或改为按批（每 N 次创建或超过阈值百分比）检查。
- **优先级**：P1（大表数场景的启动/首访延迟）。

## F4（P2）prewarm pending 去重在持锁路径上做 O(pending × 分区数) 比较

- **file:line**：`crate/engine/fkv_runtime/src/lib.rs:594-600`
  （`enqueue_table`，`VecDeque::contains` 线性查）、`:622-631`
  （`plan`，每条 pending 做 `group.contains(&tbl)`）
- **现象**：`plan` 在 `lock_prewarm_state(state)` 内执行（`tick` 的
  `(lock_prewarm_state(state).plan(table_li), false)`），pending 上限 1000、
  单个 shard 的分区组可达上万，最坏约 1e7 次 `TableIdent`（`String`）比较，
  全程持锁；`attach_shard` / `reopen` 也要抢同一把锁，Cloud 下 reopen 抖动时会
  明显卡控制面。
- **修法**：pending 改 `HashSet<TableIdent>` + `VecDeque` 双结构（去重 O(1)、
  保 FIFO、超限仍 pop_front）；`plan` 内改为按 shard 用 `HashSet` 收表，
  最后一次性转 `Vec`。
- **优先级**：P2（并发/尾延迟，规模相关）。

## F5（P2）`global_create_branch` 的 salt 用 `now_us() as u32`，约 71.6 分钟回绕重名

- **file:line**：`crate/engine/fkv_runtime/src/lib.rs:1676-1681`
- **现象**：`now_us() as u32` 每 2³² µs（约 4295 s）回绕一次；同一逻辑名在两个
  回绕周期后创建 branch 会得到完全相同的内部名 `<name>-<8hex>`，第二次落到已存在
  的 branch 上（行为取决于 `create_branch` 撞名语义，可能报错或复用旧快照）。
- **修法**：用完整 64 位时间戳派生（如取高位/混合）或 `fastrand` 随机 u32 且保证
  非 0；若必须与 C++ 8 位十六进制对齐，撞名时重新取盐并重试。
- **优先级**：P2（低频但静默的语义错乱）。

## F6（P2）attach 失败不回滚本层状态，归档 crond 会继续 fanout 到已 `Expired` 的 shard

- **file:line**：`crate/engine/fkv_runtime/src/lib.rs:1789-1820`（`attach_shard`）、
  `:1760-1775`（`record_attach` / `clear_attach`）
- **现象**：`record_attach` / `attached_li.store(true)` 只在成功分支执行；失败分支
  直接返回 `Err`，之前成功 attach 留下的 `attached_li=true` 与 `attach_ident_li`
  保持原值。而 shard 侧 `admit_attach` 在低 term 时会把 lifecycle 置为 `Expired`，
  之后所有请求返回 `ExpiredTerm`：归档 crond 每个周期仍把该 shard 当 attached
  派发，产生持续 warn；`global_archive_name` 也继续用已经失效的旧 `(branch, term)`
  构造结果名。
- **修法**：失败分支按错误分类清理本层状态（至少 `ExpiredTerm` 时清
  `attached_li` / `clear_attach`，并同步 `PrewarmState::detach`），或让 attach
  失败后向 shard 查询一次实际 lifecycle 再决定。
- **优先级**：P2（错误恢复路径的可观测性与结果名正确性）。

## F7（P3）重复常量与三处文档改名滞后

- **file:line / 现象**：
  1. `crate/engine/fkv_shard/src/lib.rs:75` 定义 `DEFAULT_YIELD_BUDGET_US = 20`，
     与 `crate/foundation/fkv_config/src/lib.rs:90` 同值重复；runtime 用的是
     `fkv_config` 版本，shard 版本仅被 `tests/yield_budget.rs:139` 引用 → 改一处
     就会静默漂移。
  2. `crate/engine/fkv_shard/src/roots.rs:21` 注释仍写 `KvOption::store_path_lut`，
     字段已改名 `store_path_lut_li`。
  3. `doc/metrics.md:120,141,146` 仍称 `ShardMetrics`，实现已改名 `ShardMetric`；
     `doc/architecture/06-gaps.md:60` 写 `standby_master_store_path_lut`，
     实际字段为 `standby_master_store_path_lut_li`。
- **修法**：删除 `fkv_shard` 的重复常量（测试改引 `fkv_config`）；同步上述三处
  文档/注释到现名。`doc/architecture/05-persistence-and-recovery.md` 的
  `ReopenQueue` 单次尝试、`manifest_limit` 8 MiB、`manifest_<branch>_<term>_<tag>`
  描述与实现一致，无需改。
- **优先级**：P3（无运行时影响，防止后续漂移）。

---

## 已核对无问题（供复审参考）

- fresh attach 的 reopen 失败通知：`write_queue_stale` 触发点只覆盖
  detach 成功 / Fresh attach / shutdown；`reopen_li.fail_all(NotRunning)` 与
  `fail_pending(gate.clear())` 顺序正确，`wake_li` 在同轮 drain_write_gate 之前
  消费，不会用旧身份重跑（测试 `fresh_attach_fails_parked_read_with_old_identity` 覆盖）。
- attach 身份 term 传递：CLI `--term` 同时传给 SDK 数据面与 `run_runtime` 的
  `attach_shard`；`manifest_path` 固定 `manifest_main_0`（sto-70），term 只影响
  data/segment 文件名且读路径按文件 id 收集全部 variant，实测跨 term 读可命中。
- `list_archive_tag_li`：`sort()` + `dedup()` 的键为 `(tag, term)`（`ArchiveEntry`
  字段序），跨分区全等去重；`.tmp` 由 `fkv_types::parse_manifest_file_name` 前置
  拒绝（有 `tests/branch_naming.rs:277` 覆盖）。
- `standby_service`：多根 master 消费 `..._lut_li`，按
  `partition_id % lut.len()` 选根，与 `RootedStore::store_of` 同式；单根 LUT `[0]`
  行为不变；`Mem` 分支 `root_li` 为空、全局列举/归档目标集合为空、`standby=None`。
- `RootedConfig` 死代码：`root_li()` / `root_of()` / `root_index()` 已删，
  `single()` 仍被 `backend.rs:117` 使用。
- 范围内无 `#[allow]`；`unwrap` / `panic!` 仅出现在 `#[cfg(test)]` 代码。


## 处置（2026-11-05）

本报告 findings 已在 r8/r9 与当前代码中修复；报告移入 `issue/reviewed/` 作为审查记录。
