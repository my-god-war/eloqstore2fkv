# r7 复审报告 · 第 1 组（store/shard/runtime/快照测试与文档）

日期：2026-11-05。范围：另一会话移入 `issue/done/` 的 6 条，逐条对照当前工作区
源码（只读 crate/，行号亲读；复审期间并行会话仍在改代码，所有被查文件按内容定位，
不匹配项为零，未触发「在途待下轮」条款）。

| 条目 | 判定 | 去向 | 摘要 |
|---|---|---|---|
| [`02`](../reviewed/02-cpp-create-branch-omits-segment-mapping.md) | 完善 | reviewed/ | 纯文档登记全数落地 |
| [`12a`](../reviewed/12a-archive-lifecycle-and-global-control-plane.md) | 完善 | reviewed/ | archive 生命周期 + 全局控制面代码/测试齐备 |
| [`27c`](../reviewed/27c-concurrent-read-write-snapshot-tests.md) | 完善 | reviewed/ | 5+3 测试在位，新增用例 5 断言覆盖 |
| [`cld-57`](../reviewed/cld-57-global-list-archive-tags-no-tag-term-dedup.md) | 完善 | reviewed/ | `(tag, term)` 去重 + 注释勘误 + 双层测试 |
| [`cld-58`](../reviewed/cld-58-standby-replica-uses-only-first-master-path-lut-unused.md) | 完善 | reviewed/ | LUT 真消费（非 `[0]`），双根 e2e 覆盖 |
| [`eng-32`](../reviewed/eng-32-workloop-serialization.md) | 完善 | reviewed/ | 读 spawn + 双通道优先级 + 预算，12 测试在位 |

新立项：无（6 条均通过，无需要立 issue 的不实/回归/缺口；查重过 `issue/` 全目录，
残余缺口均已有归属条目，见下）。

## 逐条证据（要点，全文见各文件「复审（2026-11-05 r7）」小节）

### 02 branch segment mapping 登记 → 通过

- `doc/migration.md:263` §8.2 新行在位（有意差异判定 + 双向后果 + 依据 + 跟踪列）。
- 6 处引用改指 `issue/done/02`：`COMPAT_BASELINE:13`、`doc/progress.md:6`、
  `fixtures/README.md:157`、`doc/architecture/06-gaps.md:46`（DONE 记 :45，行号
  漂 1 行，链接正确）、`04-btree-and-formats.md:89`、`doc/migration-tooling.md:198`。
- fixtures 38/38 `meta.txt` 含 `issue/done/02`，全仓 `issue/new/02` 仅剩 2 处声明
  保留的历史记录（`reviewed/01:402`、`done(现 reviewed)/eng-91:139`）。
- 遗留提示（非缺陷）：本批 6 文件移入 reviewed/ 后，上列 6 处 `issue/done/…`
  链接需下一轮 `fkv_ledger` / 文档同步统一刷新（本轮硬性约束不允许改这些文件）；
  rev-41 lane 的 `COMPAT_BASELINE:14-16` 未刷进 38 份 meta.txt 的漂移风险仍由该
  lane 收尾。

### 12a archive 生命周期与全局控制面 → 通过（含特别核对项）

- 请求族落地：`fkv_api/src/lib.rs:117` `ReopenArchive`、`:126` `DeleteArchive`、
  `:327` `Response::DeleteArchive(usize)`；`ArchiveTagInfo` `:286` +
  `fkv_store/src/lib.rs:98`，由 `list_archive_tags` 回传并有断言
  （`branch_archive.rs:352-358`）。
- store：`fkv_store/src/lib.rs:2342` `reopen_archive`（空 tag→reopen、多 term 取
  最高、replay 后写回活跃 manifest）；`:2413` `delete_archive`（全 term 幂等）。
- shard：dispatch `:2422/:2523`；`is_write_op :1375-1376`；`pri_of :92`（Delete
  低优先）；`op_of :1093/:1103`；`skips_table_open :1465-1474`；重试 park 语义
  `:1600-1606`。
- runtime：`global_archive :1438-1457`（空 tag 一次生成 + tag/name 回传）、
  `global_reopen :1462`、`global_delete_archive :1483`、salt 建分支 `:1512-1533`、
  `global_drop_table :1538-1554`（按表名过滤）、`global_list_archive_tag_li :1562`；
  `GlobalReply.tag/name :278-283`。
- 测试：`branch_archive.rs:326-439`、`archive_lifecycle.rs:72-205`（salted 名
  `snap-00000abc`、删表不波及 other）、`global_list_archive_tags.rs:51-142`、
  lib 单测 `:2439/:2494`、`cloud_e2e.rs:201-217`。
- 残余如实登记且已归属 todo（FileGc/LocalGc → `todo/12a`；空目标 name →
  `todo/eng-84`）。

### 27c 并发读写快照测试 → 通过

- `concurrent_snapshot.rs` 5 条（`:265/:309/:347/:415/:526`）：点读/floor 挂起读
  含「写已提交」反向断言（`:291-303`）；全部经 `ReadGate` 精确挂起，无 sleep。
- `gc_snapshot.rs` 3 条；新增 `gc_reclaims_old_files_immediately_without_active_snapshot`
  `:210-262`：`removed == frozen_li.len()` 当轮全回收 + 盘面逐文件确认 + `snapshot_num==0`
  前后断言——同时钉住快照并集分支的空注册表行为。

### cld-57 全局列举去重 → 通过

- `fkv_runtime/src/lib.rs:230-231` `sort()` + `dedup()`；注释 `:217-220` 已改为
  「按 `(tag, term)` 全等去重」。单测 `:2505/:2510` 跨分区全等语料 → `:2518-2540`
  只回一条；集成 `global_list_archive_tags.rs:96-122` 期望去重后 2 条。

### cld-58 standby LUT 选根 → 通过（特别核对：真 LUT，非 `[0]`）

- 消费点：`fkv_standby/src/rsync.rs:316-319`
  `master_root_li[lut[partition_id % lut.len()]]`，`remote_dir/list/sync_dir/fetch/
  verify_same` 全走它；`new_rooted` 构造期拒非法 LUT `:284-305`；跨根列举合并
  `:371-391`。runtime `standby_service` 传全量 paths+LUT `lib.rs:1032-1037`，
  空 LUT `InvalidArgs` `:1020-1028`；原 `.first()` 已消失（grep 全仓）。
- 测试：`fkv_standby/tests/standby.rs:443-487`（含非等权 LUT `[0,1,1]` 分区 2→
  `/w1`，可区分 `partition_id % root_num` 误用）、`:490-502`、`:505-525`；
  `fkv_runtime/tests/standby.rs:186-255` 双根 master e2e（先证盘面分根，两分区
  reopen 成功 + 对各自根逐字节一致）。

### eng-32 workloop 读并发 → 通过（按 32a/32b/32c 拆分范围）

- `ReadFlight`/`FuturesUnordered`（`fkv_shard/src/lib.rs:2008-2011`）+
  `MAX_READ_BATCH=32`（`:59`）+ 每读 spawn（`:1970-1973`）；写/控制先
  `finish_read_li`（`:604-636`）。
- `tx_hi/tx_lo`（`:290-292`）+ 轮预算 `:664-676`（默认 400us
  `fkv_types/src/lib.rs:37`）+ `YieldBudget` 20us（`fkv_config/src/lib.rs:57-58`）。
- `WriteGate`（`:596`）；跨分区写并发归 `todo/eng-33a`，口径一致。
- 测试 12 条在位：read_spawn 3（96 并发/慢 scan 隔离/detach 线性化）、priority 3
  （Get 插队断言 `:140-144`）、yield_budget 1、write_gate 3、read_batch 2。

## 其他

- 并行会话在本轮进行中同时把 done/ 其余 7 条（eng-84a/eng-89/eng-91/io-44/
  perf-95/rev-34/rev-41）复审移入 reviewed/，并立了 `eng-93`、`rev-46`；与本组
  无交叉冲突。done/ 现为空。
- 本轮未运行 cargo（构建锁约束）；判定全部基于源码实读。
