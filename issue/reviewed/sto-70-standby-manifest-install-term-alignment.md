# sto-70 P3：standby 追赶缺 C++ 的 manifest 落名对齐与收尾（rename 到 target_term / 清旧 term manifest / 目录 fsync）

来源：第 5 轮 runtime/cloud lane 侦查（cld-44 残留半修的确认；子代理摘录）。

## 证据

`crate/io/fkv_standby/src/lib.rs:332-356`：

```rust
let tmp_manifest = staging_dir.join(format!("{}.tmp", manifest.name));
self.source.fetch(tbl, &manifest.name, &tmp_manifest).await?;
...
rename_async(tmp_manifest, local_dir.join(&manifest.name)).await?;   // 远端原名直接安装
stats.manifest_installed = true;
let _ = remove_dir_async(staging_dir).await;
```

C++（`standby_service.cpp:882-906`）：`selected_term != target_term` 时 rename 到
`BranchManifestFileName(MainBranchName, target_term)`；`:908-946` 删除所有非
target_term 的无 tag manifest；`:948-962` 变更后 fdatasync 目录。

## 问题

Rust 体系内自洽（master 也写固定名，`check_manifest_term` 恒比 0），但：
- 跨 C++ master 时 replica 会装下 `manifest_main_<term>` 而本地
  `read_manifest` 只认固定名 → 永远 ResourceMissing、重试耗尽（01 oracle
  差分的具体断点）。
- 旧 term manifest 本地无清理，追赶多 term 后累积。

## 目标

- 安装时统一落本地活跃名（现行为）但**删除同分区其它 term 的 manifest 文件**
  （对齐 C++ 清理语义）；
- rename 后目录 fsync（复用 `ManifestIo` 的 dir fsync 基建）；
- 跨 C++ master 的 term 命名差异记录到 doc/migration.md（若决定不支持混部，
  写明判定与报错）。

## 验收

- 测试：两次追赶不同 term 后本地只剩活跃 manifest；安装后目录 fsync 被调用
  （failpoint/计数）。
- `cargo nextest run -p fkv_standby`。

## 依赖 / 重叠

- cld-44（done）只修了 fencing（check_manifest_term）；本条是其 rename/清理/
  sync 半修残留，done 单里已列为问题但未实施。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上。

## 审查结论（dragon 复核，2026-11-04）

**成立（P3→P2，跨实现可读性），已按 C++ 对齐。** 实核要点（比原描述更精确）：

- 原描述的「安装时统一落本地活跃名（现行为）」**是错的**：原实现
  `rename_async(tmp_manifest, local_dir.join(&manifest.name))` 落的是**远端原名**。
- 本地读侧的名字是**固定名**，与 replica 自身 term 无关：
  `fkv_io_compio::CompioStore::manifest_path` = `part_dir/manifest_file_name(MAIN_BRANCH_NAME, 0)`
  → `manifest_main_0`；`fkv_store::Store::prepare_attach` 对 main 分支也是
  `manifest_file = None`（活跃 manifest 交给 IO 层）。
- 因此 Rust↔Rust 恰好同名（Rust master 活跃 manifest 也是 `manifest_main_0`），
  但来自 C++ master 的 `manifest_main_<term>` 会原样落地 → 本地永远
  `ResourceMissing`、重试耗尽。**这就是 01 oracle 差分的具体断点**。
- 旧 manifest 不清理、安装后不 fsync 目录，也都属实（C++ `FinalizeRsync`
  `standby_service.cpp:882-965`：rename 到 target term + 删同分区其它无 tag
  manifest + `mutated` 时 `fdatasync(dir_fd)`）。

## DONE（2026-11-04）

`crate/io/fkv_standby/src/lib.rs`：

- 安装改为落**本地活跃名** `manifest_file_name(MAIN_BRANCH_NAME, 0)`，`rename` 原子
  替换（不学 C++ 先 `remove(dst)`，去掉「无 manifest 窗口」这一无谓风险；终态一致）。
- 新增 `remove_stale_main_manifest_li`：装完清掉同分区其它名字的无 tag **main**
  manifest。**有意收窄**（记档）：C++ 的规则是「无 tag 且 term != target_term 一律删」，
  在 Rust 里会把 branch manifest（`manifest_<branch>_0`，term 恒 0）一起删掉，
  故只删 main 分支的；带 tag 的归档两边都不删。
- 新增 `sync_dir_async`（`File::open(dir)` + `sync_data`，macOS 走 `F_FULLFSYNC`），
  每轮安装后 fsync 分区目录 —— C++ 按 `mutated` 决定，Rust 每轮都 rename 过，恒 mutated。
- `SyncStats` 增 `stale_manifest_removed` / `dir_synced`（可观测，便于断言与排障）。

**验证**：新增 `installs_cxx_named_manifest_as_local_active_name`（integrations）：
把 Rust master 的 `manifest_main_0` 改名成 C++ 风格的 `manifest_main_5`，
副本目录预置噪声（`manifest_main_3` 要删、`manifest_branchX_0` 与
`manifest_main_0_001` 要留），`target_term = 5` 同步后断言：
① 本地只有 `manifest_main_0`（无 `manifest_main_5`）；② 噪声清理结果正确；
③ `stale_manifest_removed == 1`、`dir_synced`；④ **本地 `Store` 能真正读出数据**
（`prepare_attach("main", 0)` + `reopen` + `get("a") == "A"`）。
**反向验证三次**：装回远端原名 → FAIL（`manifest_main_0` 缺失）；去掉清理 → FAIL
（`stale_manifest_removed = 0`）；去掉 fsync → FAIL（`dir_synced = false`）。
`cargo nextest run -p fkv_standby` 18 passed；`cargo clippy -p fkv_standby --all-targets
--no-deps -- -D warnings` EXIT=0。

## 未做（记档）

跨 C++ master 的**数据文件** term 仍可能不匹配：读侧
`fkv_io_compio::scan_dir(dir, branch, term)` 只认 `data_<id>_<branch>_<term>` 里
term 与自身 identity 完全相等的文件。这条不在 fkv_standby 可控范围（term 由
attach 给出），需在 doc/migration.md 的混部判定里写明「replica 的 term 必须等于
master 写数据时的 term」，或另立 issue 决定是否放宽读侧 term 匹配。

## 复审（2026-11-05 r6）

**通过。** 核实：`fkv_standby/src/lib.rs:417` `active_name = manifest_file_name(MAIN_BRANCH_NAME, 0)`（落本地活跃名，非远端原名）；:422 `remove_stale_main_manifest_li(local_dir, &active_name)`；:425 `sync_dir_async(local_dir)`（每轮安装后目录 fsync）。测试 `tests/standby.rs:710` `installs_cxx_named_manifest_as_local_active_name`：:738 `stale_manifest_removed == 1`、:739 `dir_synced`，md 记录三次反向验证。md「未做」段（跨 C++ master 数据文件 term 匹配）已如实记档为混部判定项。
