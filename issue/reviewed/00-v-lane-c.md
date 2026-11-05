# 00-v-lane-c：验证轮 lane C（gc / 恢复 / runtime / cloud / standby）

- 轮次标记：2026-11-05 v
- 输入：`issue/done/` 中 lane C 8 条
- 约束：只读源码（亲读行号），未改 `crate/` 代码/文档，未运行 cargo；仅写
  `issue/new/`、`issue/reviewed/` 并从 `issue/done/` mv
- 基线：eloqstore `82b774c`；仓库工作区（`fkv_cloud`/`fkv_shard`/`fkv_runtime`
  并行会话在途）

## 逐条去向

| 条目 | 去向 | 判定 | 关键证据（file:line，亲读） |
|---|---|---|---|
| r8c-01-gc-cloud-future-term-skip-missing | `issue/reviewed/` | 完善 | `fkv_task_gc/src/lib.rs:123`（`process_term`）、`:149`（默认 `u64::MAX`）、`:255/:263/:273`（注入）、`:279`（`term_above`）、`:532`（`term > process_term → true`）；`fkv_store/src/lib.rs:4106`（`set_process_term`）、`:4109-4111`（`term_above → ExpiredTerm`） |
| r8c-02-rev37-...-registration-lag | `issue/reviewed/` | 完善 | `05-persistence-and-recovery.md:57-61`（单次尝试 + 每读请求预算 + 外部 Reopen 不 park）；`fkv_config/src/lib.rs:306-311`（两字段注释已补）；`06-gaps.md:81-83` 同口径 |
| r9c-01-gc-expiredterm-...-window-zero-test | `issue/reviewed/` | 完善 | `fkv_task_gc/tests/variant.rs:343-350`、`:354-385`（seeded term 5、id 7<10、删除数 0）；`fkv_store/tests/gc_store.rs:266-319`（store term 5 + `manifest_main_7` → `Err(ExpiredTerm)`，文件不变） |
| r9d-01-kvstore-term-doc-...-falls-back | `issue/reviewed/` | 完善 | `fkv_sdk/src/lib.rs:35-44`、`fkv_cli/src/main.rs:11-16`、`fkv_cli/tests/cli.rs:58-64` 三处改如实；`ResourceMissing` 仅余 `fkv_sdk:126` / `fkv_cli:171` 合法 match |
| rev-37-auto-reopen-retry-policy-divergence | `issue/reviewed/` | 完善 | `fkv_shard/src/lib.rs:200/:211-218`（每请求预算）、`:1121-1144`/`:1176-1179`（外部 Reopen 无 driver 不 park）、`:1187-1216`（单次尝试，`:1213` 失败连坐）、`:1048-1056`（`fail` 全 waiter）；测试 `src:3611-3682`、`tests/cloud_reopen.rs:169/205/227`（13 compio）、`tests/auto_reopen.rs:64/88`；决策入 `05-persistence:57-61` + `fkv_config:306-311` |
| rev-38-reopen-no-per-table-prewarm | `issue/reviewed/` | 完善 | `fkv_runtime/src/lib.rs:542`、`:557-564`、`:615-625`、`:631-639`、`:659-706`（pending :678-683 不受 `done_li` 限制）、`:886-935`（列举失败仍消费、不置 done）；接线 `:1634-1640`/`:2228-2239`/`:2247-2260`；测试 `mod prewarm_tests:2592/2774`、`tests/prewarm.rs:133` |
| rev-46-standby-lazy-open-...-partition | `issue/reviewed/` | 完善（登记残余转 v-09） | `fkv_shard/src/lib.rs:1068-1070`（main + `standby.is_some()` 先 `sync_standby_partition`）、`:1872`；测试 `fkv_runtime/tests/standby.rs:284/322/388`。**登记未落**：`COMPAT_BASELINE` 无条目、`doc/` 无「懒开」 |
| sto-61b-multi-ds-shard-...-routing | `issue/reviewed/` | 完善（有意偏离，已登记） | `fkv_api` `Request` 无 `ds_shard`（grep 0）；路由按 `TableIdent::shard_index`（`fkv_runtime:493/2030`）= `partition_id % shard_num`，同 C++ `eloq_store.cpp:1020`；登记 `COMPAT_BASELINE:20-26`、`migration.md:207`、`fkv_config:11-13` |

## 判定明细

### 通过（8，已归档 reviewed）

- **r8c-01**：三层护栏代码全部在盘，`retain_variant` 的 term 门控 (:532) 前置于
  引用/在飞判定，`gc_with_scope` 熔断 (:4109) 前置于 damaged 早退 (:4130)。
  DONE 两项声明属实。
- **r8c-02**：三处文本与 `fkv_shard` 现行为逐句一致；`grep -rn auto_reopen doc/`
  仅 05 一条，与 06-gaps 无分叉。
- **r9c-01**：补齐 r8c-01 验收的「seeded 路径」+「一轮删除数 0」+ Store 级
  端到端熔断用例；`gc_store.rs` 用真 manifest 字节构造 `term: 7` 盘面，
  断言 `unwrap_err == ExpiredTerm` 且文件列表前后相等。开发期敏感度项（挪位 /
  改比较符）属可选，未入库，不阻断。
- **r9d-01**：纯注释口径修正，行为零改动；三处与 `migration.md` §8.2 /
  `read_page` 候选回退注释同源。
- **rev-37**：方案 A（严格对齐 C++）落地；「外部 Reopen 无 driver 立即回执、
  有 driver 挂 waiter」兼容 eng-39a 已定决策；决策已写入 persistence 分册与
  `fkv_config` 字段注释（特别核对项满足）。
- **rev-38**：pending 队列不受 `done_li` 限制（attach 后新分区可预热的关键），
  去重 + 有界，tick 先消费 pending 且列举失败不丢；reopen / reopen_external 接线
  到位。
- **rev-46**：懒开 main branch + standby 先 `sync_standby_partition`，失败沿
  `ResourceMissing` 挂队列不装空表；三条 standby 测试在。刻意差异登记未落，
  拆 v-09（见下）。
- **sto-61b**：范围外判定有据——C++ 能力面向宿主集成，fkv `Runtime` 单身份；
  请求路由确未按 `DsShardId`/`partition_filter` 落地，属有意偏离，且
  `COMPAT_BASELINE`/`migration.md`/`fkv_config` 三处登记齐全，验收清单保留供
  重开。特别核对项（路由 / 过滤器 / 测试）结论：按范围外关闭，登记完整。

### 不实 / 回归（0）

无。未发现声称与代码不符。两处 DONE 文本细节偏旧、不影响结论，已在对应
`复审` 小节备注：

- `rev-37`：DONE 记 cloud_reopen「12 passed」，现文件 13 个 `#[compio::test]`
  （疑后续轮次新增）。
- `rev-38`：DONE 称「`plan` 改为 `&mut self`」，现实现已重构为
  `take_plan_input` + `plan_targets`（锁外分桶去重），行为一致。

## 新立项

- [`v-09`](../new/v-09-rev46-standby-lazy-open-divergence-unregistered.md)：
  rev-46「standby 懒开先追 master vs C++ `FindRoot` 懒 replay」刻意差异未落
  `COMPAT_BASELINE` / persistence 分册（DONE 自认残留，无归口 issue）。
  已查重：`grep -rln "rev-46|懒开|FindRoot" issue/` 仅 rev-46 自身与 r7 组
  立项记录，无承接条目；`issue/new/` 现有 v-01（lane A，不同主题）。

## 在途 / 停止条件

- lane C 8 条起始时均仍在 `issue/done/`，全部逐条处理完，无跳过，无被并行会话
  移走。
- 在途观察（未阻断判定）：`fkv_store/src/lib.rs` mtime 07:49、`fkv_shard/src/lib.rs`
  07:48、`fkv_runtime/src/lib.rs` 07:52、`fkv_cloud/src/lib.rs` 07:51（近 5 分钟）。
  已 `git status`/`git diff` 核对：在途改动为 `option` 重命名、`ScanPageOption`
  形参、`fkv_cloud` r10 批次等，**未触及**本 lane 的任何锚点
  （`process_term`/`term_above`/`run_due_reopen`/`ensure_table_ready`/`plan_targets`
  /`PrewarmState`）；r8c-01/r9c-01 的 store 锚点经内容复核仍在。
- 旁路：`issue/new/r10-cloud-findings.md` 已被并行会话移至 `issue/reviewed/`
  （非 lane C，未干涉）。

## 结论

lane C：通过 8 / 不实 0 / 回归 0。新立项 1（v-09）。8 条已追加
`## 复审（2026-11-05 v）` 小节并 mv 至 `issue/reviewed/`。未改动任何 `crate/`
代码或文档，未运行 cargo。
