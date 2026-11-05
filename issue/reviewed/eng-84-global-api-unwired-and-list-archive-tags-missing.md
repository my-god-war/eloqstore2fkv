# eng-84 P2：全局 API 已实现未接线；`GlobalListArchiveTags` 整体缺失

来源：第 5 轮 `./sh/unused.js` 扫描 + 对照 C++ `include/eloq_store.h` 公开 API。

## 证据（2026-11-04 23:5x 核验）

1. `crate/engine/fkv_runtime/src/lib.rs:1040/1055/1066` 的
   `global_archive` / `global_reopen` / `global_create_branch` 被 unused.js
   判零引用（`setTest=false` 口径）；`global_drop_table`（`:1080`）仅被
   `src/lib.rs` 内 `#[cfg(test)]` 测试引用（`:1774/1788/1803`）。
   全仓（crate/ + tool/，排除 `_cpp`）无其它调用点。
2. `fkv_sdk` / `fkv` facade / `fkv_cli` 的 Cargo.toml 均不依赖 `fkv_runtime`；
   `tool/orphan_allow.tsv:6` 已把 `fkv_runtime` 列为「顶层运行时，等待入口接线」
   （挂在 12c 名下）——孤儿状态已知，但**接线动作本身没有任何 open issue 承接**。
3. C++ 公开 API（`include/eloq_store.h:1220-1223`）：
   `HandleGlobalArchiveRequest` / `HandleGlobalReopenRequest` /
   `HandleGlobalListArchiveTagsRequest` / `HandleGlobalCreateBranchRequest`
   四件套。fkv 只实现了其中三件 + 自创 `global_drop_table`
   （Drop 的 per-table 语义已由 sto-62c 定论，全局版是 fkv 扩展）。
4. **`global_list_archive_tags` 在 fkv 全仓零实现**：`fkv_metrics::Op`、
   `fkv_api::Request`、`fkv_runtime` 均无对应物。C++ 实现在
   `src/eloq_store.cpp:2133`（遍历 store_path 分区目录聚合归档条目，
   Cloud 分支列远端）。

## 目标

1. 先做入口决策（阻塞点，见「依赖」）：Runtime 的生产消费者是谁
   （`fkv` facade 重导出？`fkv_cli` 子命令？独立 server crate？）。
   决策落地后把 4 个 `global_*` 接到该入口，删除 orphan_allow 行。
2. 补 `GlobalListArchiveTags` 对等物：目标集合与 `global_archive` 一致
   （`enumerate_targets` 已可复用），聚合各分区归档条目。
   Cloud 下列远端归档（对齐 C++ 语义），`Op` 不新增（列举是控制面读，
   C++ 侧也只注册了 4 个 global 标签里的 archive/reopen；核对
   `doc/metrics.md` 后再定）。
3. `global_drop_table` 在入口接线时明确标注为 fkv 扩展（doc/migration.md
   有意偏离清单），避免后续对齐时误判为 C++ 缺失。

## 验收

- 入口 crate 依赖 `fkv_runtime` 并暴露 4+1 个全局操作；
  unused.js 对 `global_*` 零报告；orphan_allow.tsv 对应行删除。
- `global_list_archive_tags`（或等名）有单测：本地多分区归档后聚合条目正确。

## 依赖 / 重叠

- 阻塞在入口决策（同 12c §1 的 facade 侧映射悬而未决）；`12c` §2 只覆盖
  global_* 的 **Cloud e2e 测试**，不覆盖接线与 list_archive_tags，互补不重复。
- 重叠：`tool/orphan_allow.tsv`（fkv_runtime 行）、done/12-runtime-modes-api
  （已核实的运行时构造缺口，其修复即 fkv_runtime 现有实现）。

## 核验记录

- 本人实读：`fkv_runtime/src/lib.rs:1000-1090`、`fkv_sdk/src/lib.rs:1-60`、
  `fkv/src/lib.rs`、各 Cargo.toml 依赖表、`eloq_store.h:40-120/1220-1223`、
  `eloq_store.cpp:2133-2210`、`tool/orphan_allow.tsv`、unused.js 输出。

## 审查结论（dragon 复核，2026-11-04）

**四条证据逐条实核：成立（第 4 条里的 Cloud 说法有误，见下）。**

1. 成立。`crate/engine/fkv_runtime/src/lib.rs:1043/1058/1069/1083` 四个 `global_*`
   在 `crate/facade/**`、`tool/**`、`fkv_api`、`fkv_shard` 全无生产引用；
   `global_drop_table` 仅被同文件 `#[cfg(test)]`（`:1777`）引用。
2. 成立。`fkv_sdk` / `fkv` facade / `fkv_cli` 的 Cargo.toml 均无 `fkv_runtime`
   依赖（实查 0 命中），runtime 无生产消费者。
3. 成立。`fkv_types` 只有 `manifest_file_name` / `archive_file_name`，
   全仓无 `global_list_archive_tags` / `list_archive_tags`。
4. **部分不成立**：C++ `HandleGlobalListArchiveTagsRequest`
   （`src/eloq_store.cpp:2132-2213`）只 `directory_iterator` 遍历
   `options_.store_path` 下的**本地**分区目录，**没有 Cloud 分支**，也没有远端列举。
   本仓按 C++ 语义实现（本地扫描），Cloud 模式下列的是本地 cache 目录 —— 两边一致。

## DONE（2026-11-04，目标 2 部分）

补 `global_list_archive_tags` 对等物（花落 `fkv_runtime`）：

- 新增 `pub struct ArchiveEntry { pub tag: String, pub term: u64 }`（字段顺序 =
  `Ord` 顺序，直接 `sort()` 即 C++ 比较器 `(tag, term)` 语义）+
  `fn list_archive_tags(root, prefix)` + `Runtime::global_list_archive_tags(prefix)`。
- 语义逐条对齐 C++：① 直扫盘面、**不经 shard、不 fanout**，未 attach 的只读分区
  也列得出来；② 无 tag 的 `manifest_<branch>_<term>` 不算归档条目；③ `prefix`
  非空按 `rfind(prefix,0)==0` 前缀过滤（不做整段相等）；④ **不按 tag 去重**；
  ⑤ 单目录读失败跳过（控制面列举不因单个坏目录整体失败）；⑥ 非法目录名 /
  子目录 / 非 manifest 文件一律忽略。
- 已知偏离（记档）：C++ 遍历全部 `options_.store_path` 根，本仓 runtime 单根
  （多根归 `12c-followups-store-path-weights-cloud-e2e-and-store-unification`）。

**验证**：

- 单测 `global_tests::global_list_archive_tags_scans_disk_and_sorts`：手造两分区
  语料（跨 branch 同名 tag、`001`/`0012` 前缀歧义、无 tag manifest、
  `current_term_*`、子目录、非法分区名），断言全序 `[001/1, 001/2, 0012/3, 002/2]`
  与三档前缀过滤。**反向验证**：去掉 `Some(tag)` 约束 → 多出 `untagged/9` FAIL；
  排序改倒序 → FAIL；均还原后 PASS。
- 集成测试 `crate/engine/fkv_runtime/tests/global_list_archive_tags.rs`：走**真实
  写路径**（`attach_shard` + `put` + `global_archive("001")` + `global_archive("002")`
  两个分区），断言 API 结果 == 盘面真实 manifest 文件名推出的条目集合（跨 crate
  名字/布局契约），并验前缀过滤与空结果。**反向验证**：把扫描根改成不存在的子目录
  → 单测与集成测试同时 FAIL。
- `cargo nextest run -p fkv_runtime` 46 tests passed / 1 skipped；
  `cargo clippy -p fkv_runtime --all-targets --no-deps -- -D warnings` EXIT=0。

## 阻塞（目标 1，留 todo）

「Runtime 的生产入口是谁」是设计决策，不是实现细节：`fkv` facade / `fkv_cli` /
独立 server 三选一，且 4 个 `global_*` 一起接线后才能删
`tool/orphan_allow.tsv` 的 `fkv_runtime` 行。需决策后再动
（`12c` §2 只覆盖 Cloud e2e，`12a` 只覆盖 per-table `ListArchiveTags` 请求面，
均不承接这条）。

## 入口决策包（dragon 备料，2026-11-04；决策本身需架构/产品 owner 拍板）

本条剩下的只有这一件事：**谁在生产里持 `fkv_runtime::Runtime`**。下面把选项、
成本与推荐一次写清，拍板后即可直接实施。

### 现状证据（本轮实测）

- 全仓 `Cargo.toml` 里**没有任何** facade/cli 依赖 `fkv_runtime`（只有它自己与
  `fkv_runtime_cpp` 的名字命中）；全仓除 `fkv_runtime` 自身与其测试外，**没有**
  `Runtime::start*` 调用点。
- `crate/facade/fkv`（curated facade）当前只导出
  `KvOption` / `KvError,Result` / `fkv_sdk::{KvApi,KvStore}` / `ScanOption` /
  `TableIdent,now_ms` —— 没有 `fkv_runtime`、也没有 `fkv_api::Request`。
- `crate/facade/fkv_cli` 依赖 `fkv_sdk`（不是 runtime）。
- `crate/engine/fkv_runtime_cpp` 是空壳（Cargo.toml 里依赖全注释，doc 写明
  「仅差分验证，默认不进入生产依赖图」）——**不要**拿它当入口。
- `tool/orphan_allow.tsv` 仍挂 `fkv_runtime → 12c-global-fanout-and-cloud-followups
  「顶层运行时，等待入口接线」`。

### 对标 C++ 的事实

C++ eloqstore 是**库**：`HandleGlobalArchive/Reopen/ListArchiveTags/CreateBranch`
是 `include/eloq_store.h` 上的公开 API，真正的消费者是嵌入方（txservice 那一层），
eloqstore 自己不起 server。所以 Rust 侧的对等物是**库表面**，不是新造一个服务端。

### 三个选项

| 选项 | 成本 | 收益 | 风险 |
|---|---|---|---|
| A. `fkv` facade 增 `pub use fkv_runtime::{Runtime, RuntimeConfig, ArchiveEntry, GlobalReply}`（或 `pub mod runtime`） | 小：一个 Cargo.toml 依赖 + 几行 re-export + 一条端到端测试 | 与 C++ 的「库表面」对等；`Runtime` 立刻有生产消费者，orphan_allow 行可删 | 需要先定 facade 与 `fkv_sdk` 的分工：SDK 是「Local-only 简化 KV」，Runtime 是「多 shard + 模式 + 控制面」，两者都要保留则文档里必须写清「什么时候用哪个」 |
| B. `fkv_cli` 增子命令（`attach/put/archive/list-archive-tags/detach`） | 中：CLI 参数 + runtime 依赖 + 输出格式 | 运维/排障入口；可当 smoke 工具 | 只解决「有人调」，不解决「嵌入式库」；CLI 与 facade 的 API 会各自演化 |
| C. 新 `fkv_server` crate（网络服务） | 大：协议、并发模型、生命周期、鉴权 | 独立服务形态 | **与 C++ 基线不对等**，是本仓自创需求；没有需求来源就先别做 |

### 推荐

**A（必做）+ B（可选，等有运维需求再加）**；C 只在「网络服务」被列为产品需求时再议。
理由：A 直接对齐 C++ 的库形态、成本最小、且能让 4+1 个全局 API 立刻进入生产路径。

### 最小落地清单（拍板 A 之后）

1. `crate/facade/fkv/Cargo.toml` 增 `fkv_runtime`；`src/lib.rs` 增
   `pub use fkv_runtime::{ArchiveEntry, GlobalReply, Runtime, RuntimeConfig};`
   （`Runtime::global_list_archive_tags` 已在本条前半段补好，见上文 DONE）。
2. `doc/migration.md`（或 facade 的 crate 文档）写清 facade / SDK / runtime 三者分工，
   避免后来者又把 Runtime 当内部实现。
3. 端到端 entry 测试：`start_with_options` → `attach_shard` → `put` →
   `global_archive("001", n)` → `global_list_archive_tags("001")` 断言命中 →
   `global_reopen` → `detach_shard` → `shutdown`（全走 facade 导出面）。
4. `tool/orphan_allow.tsv` 删 `fkv_runtime` 行；`./tool/orphan.sh` 必须 EXIT=0。
5. `global_drop_table` 在 facade 文档里标注为 fkv 扩展（C++ 无对应，见 sto-62c）。

## 勘误 / 残余（2026-11-05）

### 已修正的语义（见 `issue/done/12a-archive-lifecycle-and-global-control-plane.md`）

- **DONE 段 ④「不按 tag 去重」是误述**：C++ `HandleGlobalListArchiveTagsRequest`
  在 `std::sort` 后 `std::unique` 按 `(tag, term)` 全等去重。fkv 已改为
  `sort()` + `dedup()`；相关期望见 `issue/done/cld-57-*.md`。
- `global_archive` 空 tag 现在只生成一次并广播全部分区，`GlobalReply::tag` /
  `GlobalReply::name` 回传结果；`global_reopen(tag)`、`global_delete_archive(tag)`、
  `global_create_branch_with_salt` 已补齐。
- `global_drop_table(tbl_name)` 已按表名过滤；上表「最小落地清单」第 5 条
  「C++ 无对应」**不准确**：C++ 有 `DropTableRequest`（按表收集分区后 fanout，
  见 `eloq_store.cpp::HandleDropTableRequest`），fkv 现与之对齐。

### 仍未完成

1. ~~**入口决策（本条主体）**：`fkv` facade / `fkv_cli` / 独立 server 三选一，
   4+1 个 `global_*` 仍未接到生产消费者，`tool/orphan_allow.tsv` 的
   `fkv_runtime` 行不能删。~~
   **已解决（2026-11-05）**：按选项 A + 最小 B 落地，见
   `issue/reviewed/eng-84a-runtime-production-entry-wiring.md`。facade 暴露
   `Runtime` / `Request` / `Response` / `StoreMode` 与 `start_runtime` /
   `start_mem_runtime`，CLI 新增 `--mode` 与控制面子命令；`fkv_runtime`
   白名单行已删，`bash tool/orphan.sh` EXIT=0。本条残余只剩下面 2、3。
2. ~~**空目标集合的 result name**：C++ 即使 0 个分区也返回
   `BranchArchiveName(branch_, ShardTerm, tag)`；fkv `Runtime` 不持有全局
   branch/term，0 个成功回执时 `GlobalReply::name` 为空。需在 runtime 记录
   attach 身份后补齐。~~
   **已解决（2026-11-05）**：见下文 DONE。
3. ~~**per-table `ListArchiveTags`**：仍只有 runtime 级盘面扫描
   （`global_list_archive_tag_li`），`fkv_api::Request` 无 per-table 变体
   （留 `12a` 第 2 条）。~~
   **已实现（2026-11-05 核对）**：`fkv_api::{Request,Response}::ListArchiveTags` +
   `fkv_shard` 分发 + `fkv_store::list_archive_tags` + `Runtime::list_archive_tags`，
   `tests/per_table_ext.rs` 覆盖；见下文 DONE。

## DONE（2026-11-05，残余 2 / 3）

### 空目标集合 result name

- `Runtime` 新增 `attach_ident_li: Mutex<Vec<Option<AttachIdent>>>`：
  `attach_shard` 成功后记录**归一化** branch + term，`detach_shard` 成功后清空。
- `global_archive`：0 个成功回执时按 attach 身份构造
  `manifest_<branch>_<term>_<tag>` 并回传；有成功回执仍用
  `ArchiveReply::name`（与分区实际 branch/term 二次一致）。
- `global_delete_archive`：与 create 同路径返回 `tag` + `name`
  （C++ Delete 也走 `HandleGlobalArchiveRequest` 的 `result_archive_`）。
- 身份选取：最低 `shard_id` 的已 attach 身份；全部 detach 回落 `(main, 0)`
  （C++ `branch_` 默认 `MainBranchName`，`ShardTerm` 对未 attach shard 返回 0）。
  已知近似：C++ `branch_` 在 `Start(branch)` 时固定、attach 不改它；fkv
  `Runtime::start` 无 branch 参数，只能用 attach 身份，detach 后回默认 `main`。
  `global_reopen` / `global_drop_table` 的 C++ 对应请求无结果名，保持为空。
- 测试 `empty_target_global_reply_uses_attach_identity`
  （`crate/engine/fkv_runtime/tests/archive_lifecycle.rs`）：未 attach 的
  `manifest_main_0_pre`、attach `MAIN`→`main` + term 7、多 shard 取最低 id、
  delete 回名、全部 detach 回落、`global_create_branch` 名不受目标集合影响。

### per-table `ListArchiveTags` 核对

无需改动，已在位：

- `fkv_api::Request::ListArchiveTags` / `Response::ListArchiveTags(Vec<ArchiveTagInfo>)`
  （`crate/engine/fkv_api/src/lib.rs:156-166`）。
- `fkv_shard`：`dispatch_read` → `Store::list_archive_tags`，`op_of` →
  `Op::ListArchiveTags`（`crate/engine/fkv_shard/src/lib.rs:1173/2192`）。
- `fkv_store::list_archive_tags`：跨 branch / term 扫 manifest 名，按
  `(tag, term)` 排序去重，不要求表已打开。
- `Runtime::list_archive_tags` 便捷入口 +
  `crate/engine/fkv_runtime/tests/per_table_ext.rs`（空表空清单；两次归档后
  `[(v1,1),(v2,1)]`）。

**验证**：

- `cargo nextest run -p fkv_runtime --lib --test archive_lifecycle
  --test per_table_ext`：最近一次 13 passed（含并发子代理新增的 2 条预热用例）。
- `cargo clippy -p fkv_runtime --all-targets --no-deps -- -D warnings`：EXIT=0
  （首次跑与并发子代理改 `fkv_mapping` 的中间态冲突，复跑通过）。
- `cargo nextest run -p fkv --test runtime_entry`：4 passed（facade 入口面）。
- `bash tool/orphan.sh`：EXIT=0（白名单仅 `fkv_io_sim` test-only）。

## 复审（2026-11-05 r7）

**判定：通过，本条全部关闭。** 残余 2 / 3 逐条核实（`fkv_runtime/src/lib.rs`
mtime 04:50、复审期间又有并行小改，行号为复审时点读数，内容与声称相符）：

- **空目标 result name 属实**：`attach_ident_li: Mutex<Vec<Option<AttachIdent>>>`
  为 `Runtime` 字段（`:962`）；`attach_ident()` 取最低 `shard_id` 的已 attach
  身份、全空回落 `(main, 0)`（`:1694-1704`）；`attach_shard` 收到
  `Response::AttachShard` 后才 `record_attach`（`:1756-1757`）；
  `global_archive` 首个成功回执用 `ArchiveReply::name`、否则按 attach 身份
  构造（`:1546-1562`）；`global_delete_archive` 同路径回 `tag` + `name`
  （`:1591-1605`）；`global_reopen` / `global_drop_table` 确无 name 赋值
  （`:1567-1584` / `:1647-1663`），与「C++ 无结果名保持为空」一致。
  DONE 新补的「已知近似」（C++ `branch_` 在 `Start` 固定 vs fkv 用 attach
  身份）在案，判定为可接受近似而非隐瞒。
- **测试抽读属实**：`empty_target_global_reply_uses_attach_identity`
  （`tests/archive_lifecycle.rs:209-258`）逐断言亲读——未 attach
  `manifest_main_0_pre`、`MAIN`→归一化 `main` + term 7、双 shard 取低 id、
  delete 回名、全 detach 回落 `main_0`、`global_create_branch_with_salt`
  `snap-00000abc` 与目标集合无关。
- **per-table `ListArchiveTags` 在位**：`fkv_api` `Request/Response::
  ListArchiveTags`（`fkv_api/src/lib.rs:158-164/351`）；`fkv_shard`
  `op_of → Op::ListArchiveTags`、`dispatch_read → store.list_archive_tags`
  （`fkv_shard/src/lib.rs:1256/2329`，DONE 引用的 :1173/2192 为改前行号）；
  `fkv_store::list_archive_tags`（`fkv_store/src/lib.rs:2484`）；
  `Runtime::list_archive_tags`（`fkv_runtime/src/lib.rs:2484`）。
  `tests/per_table_ext.rs:56-117` 断言未写表空清单（不懒建空表）与两次
  归档后 `[(v1,1),(v2,1)]`，与声称一致。
- **残余 1 复核**：`tool/orphan_allow.tsv` 已无 `fkv_runtime` 白名单行（仅剩
  2026-11-05 接线注释），入口接线经 eng-84a 交付属实。本条不再有未完成项。

验证命令因并行会话占锁未复跑，以代码 + 测试文本核对为准。
