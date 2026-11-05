# cld-58 P2：standby replica 只从 `standby_master_store_paths[0]` 追赶，master 多根时其余根的分区被漏同步并遭 drop_manifest；`standby_master_store_path_lut` 零消费

热点文件：需与 owner 会话协调（`fkv_runtime/src/lib.rs` 在另一会话 staged 改动中；本差异来自 11-04 落地的稳定代码，非半途伪差异）。

## 来源

r6 复审（2026-11-05，runtime/standby lane）核对 12b 接线与 C++ standby 路径映射时发现。

## 证据（file:line + 摘录）

Rust `crate/engine/fkv_runtime/src/lib.rs:946-956`：

```rust
let master_root = options
  .standby_master_store_paths
  .first()                       // 只取第一根
  .ok_or_else(|| { ... KvError::InvalidArgs })?
  .clone();
...
let source = RsyncSource::new(StdCommandRunner, master_root, config.source_addr()...);
```

- `crate/io/fkv_standby/src/lib.rs:60` `StandbyConfig.master_paths: Vec<String>` 存了
  全量 master 路径，但选根逻辑不存在（无 per-table 映射）。
- `crate/foundation/fkv_config/src/lib.rs:376-391` validate **接受**多个
  `standby_master_store_paths` 并生成 `standby_master_store_path_lut`——该 LUT
  全仓除 fkv_config 自身 set/clear 外**零消费**（grep 仅 `fkv_config/src/lib.rs`）。
- 放大效应：漏掉的分区在 reopen 时走 `fkv_shard/src/lib.rs:1406-1437`
  `sync_standby_partition` 的 `NotFound/ResourceMissing` 分支 → `:1430`
  `drop_manifest(tbl)`——master 侧明明存在（只是不在 root[0]），replica 却把本地
  manifest 清掉并报缺，数据视图静默残缺。

## C++ 对照（eloqstore @ 82b774c）

`src/standby_service.cpp:572-610` `RemotePartitionPath`：

```cpp
size_t remote_path_idx =
    tbl_id.StorePathIndex(options.standby_master_store_paths.size(),
                          options.standby_master_store_path_lut);
...
std::string remote_path = remote_store_paths[remote_path_idx];
remote_path.append(tbl_id.ToString());
```

C++ 每张表按 master 的路径数 + LUT（`partition_id % lut` 同款方案）算出所在 master
根，rsync 从正确的根拉取；路径配置缺失时显式 `LOG(ERROR)` 返回空而不是静默用 [0]。

## 问题

- 多根 master（生产多盘常态，`store_path=/d1,/d2:…` + 权重）下 replica 只追
  root[0] 上的分区；其余分区表现为「远端缺失」→ 本地 manifest 被反复 drop、
  reopen 永久重试失败。
- 配置面（LUT 生成）与运行面（只用 [0]）分裂：`standby_master_store_path_lut`
  是死数据，暗示接线未完成而非有意收窄；12b DONE 记档只提「replica 缺 master
  路径 → InvalidArgs」，未提多根限制，非已知偏离。

## 建议修法（择一，倾向 1）

1. `standby_service()` 多根时构建 per-root `RsyncSource` 列表，按
   `TableIdent::shard_index(lut.len())` 等价映射（复用
   `fkv_config::compute_store_path_lut` 的 LUT，与 `RootedStore::root_of` 同式）
   选源；单根行为不变。
2. 短期护栏：`standby_master_store_paths.len() > 1` 时启动即 `InvalidArgs`
   （对齐 12c 对多根本地 standby 的拒绝口径），并在 12b 记档补「多根 master
   暂不支持」。

## 验收

- 测试：master 双根（LUT 等权）各写一个分区，replica 一轮追赶后两分区都可读、
  `assert_same_files` 双根均一致；无「partition absent on master」告警。
- 若走方案 2：多根 master 配置启动被拒且错误信息可区分。
- `cargo nextest run -p fkv_runtime -p fkv_standby`。

## 甄别（2026-11-05 r6）

结论：**采纳**。逐条核实，证据成立，非半途伪差异，且未被相邻 issue 覆盖。

证据核实（均在当前工作区亲自核对）：

- `crate/engine/fkv_runtime/src/lib.rs:948-958` `standby_service()` 确只取
  `standby_master_store_paths.first()` 构造**单个** `RsyncSource`（`RsyncSource`
  持单 `master_root`，`fkv_standby/src/rsync.rs:252-276`，`remote_dir` 只拼
  `{master_root}/{tbl}.{partition}`）。`:1076` 本地根取 `root_li[0]`。
- **非伪差异**：`git show HEAD` 同位置代码相同（HEAD `961f5be`，11-05）；
  工作区对 lib.rs 的未提交 diff 仅 `yield_budget_us` 一处（:1151-1154），
  与 standby 无关——`.first()` 是已合入稳定代码。
- `standby_master_store_path_lut` 全仓 grep 仅 `fkv_config/src/lib.rs`
  （:231 声明 / :352,:384 clear / :397-398 set）——**零消费**，死数据。
  `StandbyConfig.master_paths`（`fkv_standby/src/lib.rs:60,73`）存了全量路径，
  唯一消费点是 `fkv_runtime/src/lib.rs:1082` 的 info 日志，无选根逻辑。
- `fkv_config/src/lib.rs:380-399` validate 确**接受**多根 master 路径并生成 LUT
  （多根权重不匹配/含 0 才拒）——配置面与运行面分裂属实。
- 放大链成立：本地形态下不在 root[0] 的分区 `rsync.rs:285-287` 返回
  `ResourceMissing`（「master 还没建表」）→ `fkv_shard/src/lib.rs:1430-1436`
  `drop_manifest(tbl)` + warn + `ResourceMissing` 重试——master 侧明明有数据，
  replica 却反复清 manifest、reopen 永久失败，数据视图静默残缺。
- C++ 对照属实：`eloqstore/src/standby_service.cpp:573-614`（82b774c）
  `RemotePartitionPath` 按 `tbl_id.StorePathIndex(paths.size(), lut)` 每表选根
  （`src/types.cpp:37-46`，`partition_id % lut.len()`，与 Rust
  `fkv_shard/src/roots.rs:59-62` `RootedConfig::root_index` 同式）；配置缺失
  显式 `LOG(ERROR)` 返回空，不静默用 [0]。Rust 确实偏离 C++。
- 现实触发面：Rust `Local` 模式多根 master 自 12c 起合法
  （`backend_of → local_backend → Backend::LocalRouted`），C++ eloqstore master
  多根本就是生产常态；两种形态下 replica 均会漏同步。非纯假设。

与相邻 issue 的边界：

- `reviewed/12c-followups-store-path-weights-cloud-e2e-and-store-unification.md`
  （store_path LUT 已落地）：落地的是 **replica 本地** `store_path` 多根选根
  （`roots.rs` `RootedStore`）+ standby 模式**本地**多根显式拒绝
  （`fkv_runtime/src/lib.rs:910-917` "standby multi-root store_path
  unsupported"）——管的是本地侧，**不涉及 master 侧 `standby_master_store_paths`
  的选根**，与本条正交互补。
- `reviewed/12b-standby-runtime-wiring.md`（:167,:204）：复核确认了
  `[0]` 接线「按计划落地」，但未把「master 多根不支持」记为已知偏离；
  本条即补这个缺口，不算重复。
- `reviewed/cld-44`（term fencing/manifest 选择）、`cld-45`（rsync 逐文件阻塞）、
  `done/sto-70`（manifest 安装 term 对齐）：均为 standby 其他主题，无重叠。

开发计划（**需与 owner 会话协调：`fkv_runtime/src/lib.rs` 为并行会话热点文件，
合并前 rebase 并逐 hunk 复核**）：

1. `crate/engine/fkv_runtime/src/lib.rs` `standby_service()`：多根时构建
   `Vec<RsyncSource>`（每根一个），消费 `options.standby_master_store_path_lut`，
   选根用 `partition_id % lut.len() → lut[slot]`（与 `RootedConfig::root_index`
   / C++ `StorePathIndex` 同式）；单根行为不变。
2. `crate/io/fkv_standby/src/lib.rs`：`RsyncStandbyService` 需接受 per-root
   source（或 source 选择器注入 `RemoteSource` 层，`RemoteSource::list/sync_dir`
   签名不变、实现内部按 tbl 选根，改动面更小——倾向后者）。
3. 若短期只走护栏：`standby_master_store_paths.len() > 1` 时启动即
   `InvalidArgs`（对齐 `backend_of` 对本地多根 standby 的拒绝口径），并在
   12b 记档补「多根 master 暂不支持」。
4. 测试（`crate/engine/fkv_runtime/tests/standby.rs`）：master 双根等权 LUT
   各写一个分区（`partition_id % 2` 分属两根），replica 一轮追赶后两分区可读、
   `assert_same_files` 对应根一致、无 "standby partition absent on master"
   告警；护栏路径加「多根配置启动被拒」断言。`cargo nextest run -p fkv_runtime
   -p fkv_standby`。

## DONE（2026-11-05）

实现（采用方案 1：按 LUT 选根；单根行为不变）：

- `crate/io/fkv_standby/src/rsync.rs`
  - `RsyncSource` 由单 `master_root` 改为 `master_root_li` +
    `master_path_lut_li`；新增 `new_rooted()`（空根列表 / 空 LUT / 越界 LUT
    在构造期返回 `InvalidArgs`），单根 `new()` 固定 `lut = [0]`。
  - `remote_dir()` 按 `lut[partition_id % lut.len()]` 选根，与
    `RootedConfig::root_index` / C++ `TableIdent::StorePathIndex` 同式。
  - `list_table_li()` 合并全部 master 根并按 `(表名, 分区号)` 定序去重；本地
    不存在的根视为「暂无表」，ssh 根命令失败仍上抛。
- `crate/engine/fkv_runtime/src/lib.rs` `standby_service()`
  - 消费 `standby_master_store_paths` + `standby_master_store_path_lut_li`
    构造 `RsyncSource::new_rooted(...)`；LUT 为空视为配置错误（`InvalidArgs`），
    不再静默退回 root[0]。启动日志同时打印 paths 与 LUT。

测试：

- `crate/io/fkv_standby/tests/standby.rs`
  - `rsync_source_selects_master_root_by_lut`：双根等权 LUT，分区 1/2 分别用
    `/m1/demo.1` / `/m0/demo.2`；非等权 LUT `[0,1,1]`（1:2）下分区 2 必须走
    `/w1`（可区分「误用 `partition_id % root_num`」的实现）；单根 `new()` 仍
    恒用唯一根。ssh 命令参数逐字断言。
  - `rsync_source_rejects_invalid_master_lut`：空根 / 空 LUT / 越界 LUT 启动即拒。
  - `rsync_source_lists_tables_across_master_roots`：双根目录列举合并定序。
- `crate/engine/fkv_runtime/tests/standby.rs`
  - `replica_catches_up_multi_root_master_by_lut`：master 本地双根等权
    （分区 1 → `master_b`、分区 2 → `master_a`，先断言盘上确实分根），
    replica 配双根 master 路径 + 本机 rsync；`list_standby_partition`
    同时列出两根上的分区，两分区 reopen 均成功、点读命中、
    `assert_same_files` 对各自根逐字节一致。修复前分区 1 会因 root[0] 无此
    分区报 `ResourceMissing` 并走 `drop_manifest`。

验证：

```
cargo nextest run -p fkv_runtime --test standby
cargo nextest run -p fkv_standby
cargo clippy -p fkv_standby -p fkv_runtime --all-targets -- -D warnings
```

以上均通过（runtime standby 3/3；fkv_standby 23/23）。

风险 / 已知边界：同一分区若意外同时存在于两个 master 根，仍只按 LUT 拉
路由根，另一份不参与同步（与 C++ `RemotePartitionPath` 单根语义一致）；
`catch_up()` 对本地不存在的根静默视为空表。

## 复审（2026-11-05 r7）

结论：**通过（LUT 真消费，非 `[0]` 回退）**。核当前源码（均亲读）：

- `fkv_standby/src/rsync.rs:262-268` `RsyncSource` 持 `master_root_li` +
  `master_path_lut_li`；`new()` 单根恒 `lut=[0]`（`:271-277`）；`new_rooted()`
  构造期拒绝空根/空 LUT/越界（`:284-305`）；**选根消费点**
  `master_root_of`：`master_root_li[lut[partition_id % lut.len()]]`（`:316-319`），
  `remote_dir`/`list`/`sync_dir`/`fetch`/`verify_same` 全走它（`:321-369`）；
  `list_table_li` 合并全部根 + `(表名,分区号)` 定序去重、本地空根跳过
  （`:371-391`）。
- `fkv_runtime/src/lib.rs:1013-1039` `standby_service()`：消费全部
  `standby_master_store_paths` + `standby_master_store_path_lut_li` 构造
  `RsyncSource::new_rooted`（`:1032-1037`）；LUT 为空即 `InvalidArgs`
  （`:1020-1028`）。全仓 grep 确认原 `.first()` 选根已消失，
  `standby_master_store_path_lut_li` 消费点 = runtime `:1021/:1035` +
  rsync `:316-318`。
- 测试抽读：`fkv_standby/tests/standby.rs:443-487`
  `rsync_source_selects_master_root_by_lut`（等权 LUT 分区 1→`/m1`、2→`/m0`；
  非等权 `[0,1,1]` 分区 2→`/w1`，可区分误用 `partition_id % root_num`；单根
  `new()` 不变；ssh 参数逐字断言）；`:490-502` 拒空根/空 LUT/越界；
  `:505-525` 双根列举合并。`fkv_runtime/tests/standby.rs:186-255`
  `replica_catches_up_multi_root_master_by_lut`：先断言盘面确实分根
  （`:218-222`），`list_standby_partition` 双根齐列（`:231-233`），两分区 reopen
  均成功且 `assert_same_files` 对各自根逐字节一致（`:235-247`）。
