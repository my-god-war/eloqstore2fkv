# cld-44 standby 同步不做 term fencing，manifest 选择与安装语义和 C++ 不一致

## 位置
- Rust: `crate/io/fkv_standby/src/lib.rs:181-274`（`sync_table`：`:186` `select_manifest(&remote_li, MAIN_BRANCH_NAME)` 选最新 manifest，`:239-270` 拉到 staging 校验后 `:270` `rename_async` 原样安装）；`:308-325`（`select_manifest` 按 `(term, modified)` 取最大，无 target_term 概念）
- C++: `eloqstore/src/standby_service.cpp:821-964`（`RunPrepareManifestJob`：`:860-863` 对 `term > job.target_term` 返回 `ExpiredTerm`；`:882-906` 把选中的 manifest 重命名为 target_term 名；`:908-946` 清理非 target_term 的旧 manifest）

## 现状
Rust `sync_table` 拉取并安装 manifest 时：
1. `select_manifest` 只按文件名里的 `(term, modified)` 取最大，**不与 replica 当前 process term 比较**，没有 `ExpiredTerm` 判定。
2. 安装时把远端 manifest **原样** rename 到本地同名文件（`:270`），不重命名到目标 term，也不清理本地遗留的其它 term 的 manifest。

## C++ 实现
C++ `RunPrepareManifestJob`：
- `:860-863` 遍历本地 manifest，若 `term > job.target_term` 直接返回 `ExpiredTerm`（防止 replica 接受超过自身 term 的 manifest）；
- `:882-906` 选中的 manifest 若 `selected_term != target_term`，先 `remove` 目标名再 `rename` 到 `BranchManifestFileName(MainBranchName, target_term)`；
- `:908-946` 删除所有非 target_term、非 archive 的 manifest，保持目录只保留目标 term 的活跃 manifest；
- `:948-962` 变更后 `fdatasync` 目录。

## 差异与风险
- **缺少 term fencing（P1）**：replica 可能接受并安装 term 大于自身 process term 的 manifest。若 master 侧发生 term 推进/回退（failover），replica 会装上一个“未来 term”的 manifest，本地 mapping/root 与自身 term 不一致，切换后数据视图与 term 语义错乱。C++ 明确拒绝。
- **manifest 命名不一致**：Rust 安装的是远端原名（如 `manifest_main_9`），若 replica 的 active term 是 7，本地活跃 manifest 名与 term 不匹配，`get_manifest`/replay 按 term 定位时可能找不到或选错。C++ 统一重命名为 target_term。
- **旧 manifest 残留**：Rust 不清理非目标 term 的 manifest，目录里累积多份历史 manifest，与 C++ 的“只留 target_term”不一致，影响 list/清理与磁盘占用。

## 对齐建议
- `sync_table` 增加 target term 参数，`select_manifest` 后校验 `selected_term <= target_term`，否则返回 `ExpiredTerm`（对齐 `standby_service.cpp:860-863`）。
- 安装时把选中 manifest 重命名为 `manifest_<branch>_<target_term>`（对齐 `:882-906`），并清理本地非 target_term 的非 archive manifest（对齐 `:908-946`），变更后 fsync 目录。
- 补测试：远端 manifest term > replica target_term 时 `sync_table` 返回 `ExpiredTerm`；安装后本地目录只保留目标 term manifest。

## 性能优化建议
无

## 严重度
P1（standby 缺 term fencing / manifest 安装语义不一致）

## 审查结论（2026-11-04）

**采纳（P1），转 todo**。

- 成立：`select_manifest` 无 target term 校验，安装保持远端原名，且不清理
  非目标 term manifest；与 C++ `RunPrepareManifestJob` 不一致。
- 实施需先给 `StandbyConfig` 增加 `target_term`（或复用 `CURRENT_TERM`），
  会影响 `fkv_standby` 公共配置与测试夹具。
- 与 `cld-45`（分区级 rsync）共享 `sync_table` 改动，建议同一 owner 一起做。

## 进展（2026-11-04，fencing 完成）

- `StandbyConfig` 增加 `target_term`（0 = 不做 fencing）。
- `sync_table` 选中 manifest 后调用 `check_manifest_term`：term 大于
  `target_term` 返回 `ExpiredTerm`，archive manifest 不受限。
- 新增 `manifest_term_fencing` 单测。
- 未做：安装时重命名为 target term、清理非 target term manifest
  （fkv main manifest 固定 `manifest_main_0`，需先统一命名约定）；
  `target_term` 尚未从 `KvOption` 透传。

## 复审（2026-11-04）

**结论：done（fencing 已落地；另两个子项不成立，不实现）。**

核验（当前源码为准）：

- term fencing 已实现：`crate/io/fkv_standby/src/lib.rs:59`（`StandbyConfig::target_term`）、
  `:192`（`sync_table` 选中 manifest 后 `check_manifest_term`）、`:332-338`
  （term > target_term → `KvError::ExpiredTerm`，archive（tag 非空）不受限）。
  测试 `fkv_standby::standby::manifest_term_fencing` 覆盖（10/10 通过）。
  与 C++ `eloqstore/src/standby_service.cpp:860-863` 语义一致。
- 子项「安装时重命名为 target_term」**不成立**：Rust 盘面 main manifest 名固定为
  `manifest_main_0`，非 term 化。证据：`crate/io/fkv_io_compio/src/lib.rs:1339-1341`
  （`manifest_path` 硬编码 `manifest_main_0`）、`crate/engine/fkv_store/src/lib.rs:1705`
  （`manifest_file_name(&normalized, 0)`）；全仓仅 `manifest_main_0`（另见
  `fkv_store/tests/branch_archive.rs:200`、`fkv_cloud/tests/segment_manifest.rs:249`）。
  按 issue 建议重命名为 `manifest_<branch>_<target_term>` 会让本地 reopen /
  replay 找不到活跃 manifest（`manifest_path` 只认 `manifest_main_0`），是回退而非修复。
  C++ 之所以能 rename 到 `target_term`，是因为其 store 本身按 term 命名 manifest
  （`BranchManifestFileName(MainBranchName, store_->Term())`，`standby_service.cpp:628`）。
  根因在 io 命名层，且 `fkv_io_compio/src/lib.rs` 属并发会话热点文件，本 lane 禁改。
- 子项「清理非 target_term 的 manifest」**不成立**：同上命名约定下，target_term != 0 时
  该逻辑会删掉唯一活跃的 `manifest_main_0`；且 branch/archive 生命周期已由
  `fkv_store`（`delete_branch`、`prune_archives`）统一管理，standby 侧重复 GC 会与之冲突。
- 「`target_term` 未从 KvOption 透传」：`KvOption` 无 term 字段（见
  `crate/foundation/fkv_config/src/lib.rs:17-83`），且 `fkv_standby` 当前无生产调用方
  （全仓 grep 仅测试构造），透传无来源，不做无谓接线。

遗留兼容性提示（不属本 issue，需 io/store 命名层解决）：若 master 是 C++ 实现
（产出 `manifest_main_<term>`），Rust replica 装进去后本地 store 仍读 `manifest_main_0`，
跨实现 standby 需先统一 main manifest 命名。

## 复审（2026-11-05 r6）

**通过（fencing 子项）。** 当前源码核实：

- `fkv_standby/src/lib.rs:64` `StandbyConfig.target_term`；:300 `sync_table_with_term`、:313 `check_manifest_term(&manifest.name, target_term)` 在选中 manifest 后立即校验（term > target 拒 `ExpiredTerm`，tag manifest 豁免）；:292 `sync_table` 退化为配置 term。
- 测试 `fkv_standby/tests/standby.rs:299` `manifest_term_fencing` 在位。
- 两个不成立子项（rename 到 target_term、清理非 target manifest）的判定在 r5 已复核且与 sto-70 的最终落地一致：安装改落本地活跃名 `manifest_file_name(MAIN_BRANCH_NAME, 0)`（`fkv_standby/src/lib.rs:417`）+ `remove_stale_main_manifest_li`（:422），属后续 sto-70 的收窄实现，不与本条冲突。

## 复审（2026-11-05 r7，归档豁免显式化）

**通过。** r6 的「tag manifest 豁免」此前只体现在代码分支上，容易被误读成
「忘了校验归档」。本次补两处一致性：

- `fkv_standby/src/lib.rs` `check_manifest_term` 文档写明豁免依据：归档是
  不可变历史快照，只在 `ReopenArchive(tag)` 显式点名时安装，用远端原名、不替换
  活跃 manifest、不改进程 term 视图；归档 term 高于 replica 当前 term 是合法的
  跨 term 回溯，fencing 反而会拒绝用户显式请求。
- 安装分支注释与实现对齐：归档落**远端原名 `manifest_<branch>_<term>_<tag>`**
  （供 `reopen_archive(tag)` 按名选中），只有活跃 manifest 落 `manifest_main_0`；
  旧注释同时写了两种相反语义，已删除。

口径：活跃 manifest（tag 为空）严格 `term <= target_term`；归档 manifest 豁免。
