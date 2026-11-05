# r6 复审报告：读 / 扫 / GC / 恢复 lane

日期：2026-11-05。范围：`crate/engine/{fkv_task_read, fkv_task_scan, fkv_task_gc,
fkv_gc, fkv_recovery, fkv_task_reopen, fkv_task_prewarm}` 及其配套改动
（`fkv_btree` IndexCache 族、`fkv_mapping` journal/统计、`fkv_store` 读扫/GC/TTL 入口）。
全程只读源码，未改 crate/、未跑构建；行号均来自本人亲读。

## 一、done/ 复核结果：50 条，全部通过

每条均按 md 内声称的实现要点与测试文件逐一核对源码与测试断言（关键断言抽读），
在文件末尾追加「## 复审（2026-11-05 r6）」小节后移入 `issue/reviewed/`。

### 读 / 扫路径（14）

| 条目 | 核验要点（file:line 亲读） |
|---|---|
| eng-64（原）+ eng-64a | `fkv_task_read/src/lib.rs:287-289` 过期判定先于 `iter_value`（overflow/解压）；`fkv_store:616-626` `read_ctx_at`；`tests/ttl.rs:133` 断言过期读 `read_num` 与内联小 value 相同 |
| eng-65（原）+ 65a/65b | `doc/migration.md:218` 契约行；`fkv_task_scan:1-5` 模块文档；`:176-185` 环检测（快照槽位上界 + 自环）；`tests/scan.rs:217` p0→p1→p0 → `Corrupted` |
| eng-66（原）+ 66a/66b | `CacheCounter`（fkv_btree:188，miss 回填计数 :317-319）；`add_cache_counters`/`set_index_cache_bytes`（fkv_metrics:397/416）；`sync_cache_metrics`（fkv_shard:1243）；`with_index_cache_bytes`（fkv_store:295）；测试 index_cache.rs:141、config.rs:357 |
| eng-67 | `IndexNode.file_page_id`（fkv_btree:84）、`retain_mapped`（:452）、`install_replay_state` same_identity + retain（fkv_store:1790-1811）；测试 index_cache.rs:266（复用后仅 1 次 data 页 IO）/301（term 变化整丢） |
| eng-70a/70b | `IndexNode { leftmost, fpid, page }` + `new_prevalidated` 惰性 seek（fkv_btree:82-111、fkv_page/index.rs:252）；单次构造在 `descend_step`（:245-249） |
| eng-86 | `descend_step`/`Descend`/`descend_child` 一处定义（fkv_task_read:201-262），单次 `get`；三入口统一；`tests/read.rs:145` 断言 (0,2)→(2,2) |
| eng-87 | `seek_index`/sleeper 族/`live_page_li` 全仓 grep 零命中；`fkv_task_reopen` 仅剩 `AutoReopenOption` |
| perf-84 | `scan_entry_opts` 透传 `max_entries`+`ttl_filter_now_ms`（fkv_store:1004-1025）；`tests/scan_pushdown.rs:22` 断言 `limited_reads*8 < full_reads` |
| perf-85 | `keys_only`（fkv_task_scan:31-36）+ `truncate_from`（fkv_store:1938-2001，**已超出 DONE**：TRUNCATE_FROM_BATCH 分批墓碑已落地）；三测试断言读页数/语义/segment 归零 |
| perf-86 | `CLEAN_EXPIRED_MAX_ENTRIES=256`（fkv_store:46/2041）、`next_expire_ts` 压 `now`（:2053）、内联清理 `log::warn!`（:1339-1342）；`tests/clean_expired_bound.rs:134/197` |
| perf-88 | ② `fkv_task_gc:388-393` 按字节相同跳过活跃 manifest 第二遍 replay；③ `fkv_task_scan:180/204-212` 首页 seek；① `fkv_large:249` 保持 `vec![0u8]`（reject 一致） |
| 27（原）+ 27b | `MappingSnapshot`（fkv_mapping:955）、代际门（fkv_btree:290-298）、`retag`（:432，fkv_store:1810/2358）；测试 `generation_gates_hits_and_retag_refreshes`（:1062） |

### 写侧交叉（按任务点名，4）

| 条目 | 核验要点 |
|---|---|
| rev-02 | `check_batch_keys`（fkv_store:1295-1304）+ `put_at` 首行（:1325）；`tests/store.rs:48` |
| rev-03/03a/03b | `READ_RETRY_TIMES` + `is_retryable`（fkv_io_compio:92/1955）；reopen 窄集注释在 `fkv_shard:1009`。注：rev-03b 的 DONE 段是 03a 的复制、其引用的 `auto_reopen_with_size_with_sleeper` 已被 eng-87 删除——注释目标本身已达成，瑕疵记录在案 |
| perf-87 | `split_lens`/`pick_split`（fkv_task_write:1300/1252）+ `EntrySize`（fkv_page/data.rs:54/67/95）；测试 :2426/:2508。实现在写路径 crate，交叉核验 |

### 恢复 / reopen（7）

sto-51（`split_manifest_with_len` valid_len，fkv_task_reopen:21；fkv_store:1726-1727 双路
reset_manifest）、sto-51a（`replay_manifest_bytes_with_len`:118、`reset_manifest_file`
fkv_io:108、open_branch fkv_store:1668；测试 compio_manifest.rs:89/branch_store.rs:180）、
sto-53（`bump_max_fp_id` 增量维护 fkv_recovery:114-118/:173-193；测试 recovery.rs:108/136/163）、
sto-68（`branch_metadata_opt`:87-96、`U32_BYTES`:48、缺段默认/保留；测试 recovery.rs:230/245/260/290）、
sto-64 缺口已由 sto-64a 处置（见跳过说明）。

### GC / 维护（18）

sto-54/54a（`InflightGuard` branch_li/RangeEntry/partition_point/保守保留，
fkv_task_gc:64-79/211/240-243；inflight.rs:63/119、gc_inflight.rs:48）、
eng-88（`entry_inflight` 消费 `entry.term`/`newest_term`，:246-260；inflight.rs:90/107 双向锁死）、
sto-55/55a/55b/55c（`prune_superseded_manifest_files` fkv_io:197、`try_remove_empty_partition_dir`
:538、`Store::gc` 尾部 fkv_store:3348/3352、`cleanup_empty_partition`:3197；gc_cleanup.rs 4 用例、
truncate_drop.rs 6 用例、manifest_prune.rs 5 用例）、
sto-56/56a/56b/56c（`mapping_count`/`space_size`/`skip_to_next_file`/`update_stat_by_scan`
fkv_mapping:144/400/417/893、`compact_plan` fkv_gc:27、`segment_compact_plan`:74、
`live_page_li_by_file` fkv_task_gc:295；gc.rs:13、gc_store.rs:151/236）、
eng-68/68a/68b（journal `changed_len` fkv_mapping:560/699/723、clear 接口 :761-775、
compact/truncate 出口清空 fkv_store:2354/1920/1924、批出口 debug_assert :646-655；
mapping.rs:399、index_cache.rs:69/121）、
eng-69/69a/69b/69c（Clock 两段+FIFO fkv_btree:386-413、`CacheLedger` 均分 :141-172/:343-353、
`with_index_cache_total_bytes` fkv_store:309；config.rs:378、options.rs:111）。

### prewarm（3）

cld-47（DiskFull 预判 fkv_task_prewarm:129-131、`file_entry_stream`:61-101、
`cache_remained` fkv_cloud:540；prewarm.rs:144/171）、
cld-47a（`PrewarmCrond` fkv_runtime:616/1240、`set_identity`/`remote_partition_page`
fkv_cloud:2186/2198）、
cld-54（List/Pull 双 break fkv_task_prewarm:151-162；prewarm.rs:240 断言
`files_failed==1`、`total_files_listed<32`）。

## 二、跳过（非本 lane，按内容判断）

- `sto-57/57a/57b/57c`、`sto-58`：`fkv_config` 配置面（INI/validate/derive_mode）。
- `sto-60`：`fkv_metrics` 指标契约。
- `sto-63/63a/63b`：COMPAT_BASELINE / migration.md / fkv_ledger 文档口径。
- `sto-64/64a`：`install_external_snapshot` 实现在 `fkv_store`/`fkv_cloud`/`fkv_shard`/
  `fkv_api`（恢复语义相邻，但主实现 crate 均非本 lane 7 crate）。
- `sto-70`：`fkv_standby`（runtime/cloud lane，r5 来源即该 lane）。
- `io-41/42`、`io-43`：`fkv_io*`（io/compio lane；perf-88 ① 的 reject 依据 io-41 已交叉引用）。
- `perf-8x` 其余、写路径族（sto-65/66 等）：写路径 lane。

不确定项：无。

## 三、新立项：0 条

本轮对 lane 内 7 个 crate 全文通读 + 配套 `fkv_btree`/`fkv_mapping`/`fkv_store` 关键段
抽读，与 C++（eloqstore 82b774c）对照后未发现「未被现有 issue 覆盖且达到立项门槛」的
差异。判定依据：

1. 读/扫路径的并发化、快照化、分批化差异已由 todo 的 27c / eng-32b / eng-34b 覆盖
   （27b 已完成并本轮复核）。
2. 相邻新发现已由其他 lane 会话先行立项：`new/rev-37`（auto-reopen 重试策略）、
   `new/rev-38`（reopen 无 per-table prewarm）、`new/rev-39`（GC file id 粒度跨 branch
   泄漏）——与本 lane 候选去重后无剩余。
3. 三个未达门槛的小点记录为遗留风险（下节），不立项。

## 四、遗留风险（不立项，记录在案）

1. **`PageMapper::page_cnt()` 孤儿 API**（`fkv_mapping/src/lib.rs:871`）：27b 快照化后
   scan 改用 `snapshot.data_tbl().len()`，该 pub 方法全仓零调用。清理需动
   `fkv_task_read`/`fkv_task_scan`（并行会话热点），与 owner 同批处理。
2. **prewarm 统计口径**：`prewarm_table` 把 manifest 缺失计入 `missing_num` 并汇入
   `files_skipped_missing`（`fkv_task_prewarm/src/lib.rs:179-188`），与 C++
   「只计数据文件」口径不同（cld-54 原文第 3 点的附带项，DONE 未单独处置）。
   仅统计面，无行为影响。
3. **truncate 子树整删长期项**：perf-85 的 C++ `TruncateIndexPage`/`DeleteTree`
   结构性整删仍以「sto-56b 之后评估」挂起（sto-56b 本轮已复核为 done）；当前
   keys_only + 分批墓碑已消除 value 物化，剩余为写放大优化空间。
4. rev-03b 的 DONE 段复制错误（内容为 rev-03a）与其引用函数已被 eng-87 删除——
   注释实体在 `fkv_shard/src/lib.rs:1009`，行为无影响，md 已在 r6 复审小节更正记录。

## 五、最终判定

**完善**（就本 lane 已知差异面而言）：

- 新立项 0 条；
- lane 内 done/ 条目 50/50 复核通过（含 2 条交叉点名的写侧条目与 3 条 cloud/runtime
  交叉条目，均注明归属）；
- 已知未覆盖差异：无（todo 中 27c/eng-32b/eng-34b 覆盖读扫路径剩余项，
  rev-37/38/39 覆盖相邻新发现，均已确认不与本章重复）。

前提说明：判定基于 r6 时点源码（27b 已合入、并行会话仍在改 fkv_task_read/scan）；
上述热点文件后续合入若引入新差异，需下一轮复审覆盖。
