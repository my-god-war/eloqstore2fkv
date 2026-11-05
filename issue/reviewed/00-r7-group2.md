# r7 第 2 组复审报告（io / bench / baseline / 恢复类，2026-11-05）

复核范围：另一会话移入 `issue/done/` 的 7 条，逐条按 DONE 声称到当前源码核对
（行号均亲读；复核时点 03:52–03:57）。并行会话仍在热改 fkv_store / fkv_io_compio /
fkv_cloud / fkv_runtime / fkv_cli / fkv_btree（mtime 03:48–03:55），本轮所有
「声称 vs 代码」核对均以内容锚点定位，未发现声称失实，故无「在途待下轮」判定。

## 结论一览

| 条目 | 判定 | 去向 |
|---|---|---|
| eng-84a Runtime 生产入口接线 | 完善 | reviewed/ |
| eng-89 compact 前 data fsync | 完善 | reviewed/ |
| eng-91 COMPAT_BASELINE 证据行 + meta.txt | 完善 | reviewed/ |
| io-44 分区目录 entry fsync | 完善 | reviewed/ |
| perf-95 bench test_secs 稳态窗 | 完善 | reviewed/ |
| rev-34 cloud reopen 远端 adopt | 完善（Cloud 范围） | reviewed/ |
| rev-41 MemCachedPage 裁剪登记 | 完善（B 路径登记） | reviewed/ |

新立项 2 条（去重后）：`issue/new/eng-93-*`（COMPAT_BASELINE 登记行欠账 +
README 过期路径）、`issue/new/rev-46-*`（Standby 懒开不同步）。

## 逐条证据

### eng-84a Runtime 生产入口接线 — 完善

- fkv_runtime：`Default for RuntimeConfig` :72（`large_value_threshold: 0` :86）、
  `new(root)` :102、`start_mem` :1085、`start_impl(backend_override)` :1089-1092/:1150、
  Mem `root_li` 空 :1158。
- facade fkv：重导出 :20-29、`start_runtime` :35 / `start_mem_runtime` :42。
- fkv_sdk：`pub use fkv_btree::BatchEntry` :15。fkv_cli：`--mode` 七值 + `--term`
  (0 拒绝) main.rs:77-84/:390-393，5 个控制面子命令 :434-454/:324-353，依赖仅
  facade（Cargo.toml）。
- `tool/orphan_allow.tsv` 仅剩 fkv_io_sim。测试：runtime_entry.rs 4 条
  (:24/:60/:92/:111，mem 断言不落盘 :108)、modes.rs mem :136、cli.rs 6 条
  (:18/:58/:121/:144/:156/:168)。

### eng-89 compact 落 manifest 前缺 data fsync — 完善（重点核对项）

- `compact_file_li`（fkv_store/src/lib.rs:2504）:2546-2550 `sync_data → append_mapping_log`
  组合，失败走 `settle_mapper_batch(false)` :2565；`compact_segment_file_li`
  (:3350) :3391-3395 同构；全文 `sync_data` 仅 put :1506 / clean_expired :2244 /
  两 compact :2547/:3392 四处，无漏网。
- `data_sync_num` 计数在 `fail_point!("SyncData")` 之前（fkv_io_compio:2185-2187），
  访问器 :985。
- 测试 durability_sync_order.rs：两路径注入 `SyncData@1!err=IoFail`（:73/:159），
  断言 IoFail + 计数恰 +1 + manifest 字节不变 + max_fp_id/root 回滚 + dirty 非空 +
  重试成功数据完整（:77-106/:163-196）——真实覆盖顺序，非注释式。

### eng-91 COMPAT_BASELINE 证据行 — 完善（含已知风险核实）

- COMPAT_BASELINE 旧措辞（无 payload / oracle 未实现 / issue/todo/01- 等）全 0 命中；
  证据区 :14-17、L1 前置 :18-20 均为事实口径、引用 reviewed/。
- **38/38 meta.txt 与当前 COMPAT_BASELINE flatten（`" | "` 连接）逐字节一致**（脚本
  全量比对 0 mismatch）；fixtures 22+16 case、6 skipped、16+16 真实 payload。
- 已知风险裁定：rev-41 的页缓存登记行**未进入** COMPAT_BASELINE（其 DONE 明言待
  协调者补），故「:14-16 页缓存条目未刷进 meta.txt」的失同步**不存在**——行没加，
  烤文一致。欠账转 eng-93。
- 附带：fixtures/README.md:154 指向 `issue/done/eng-91-…md`，本次归档后过期，
  复审会话无权改 fixtures/，已并入 eng-93。

### io-44 分区目录 entry fsync — 完善

- `ensure_dir`（fkv_io_compio:1825-1853）：建目录 + fsync root（:1842）+ root 新建
  时 fsync 父（:1843-1850）后才记 `dir_ready`（:1851）；`sync_dir_entry`
  (:1859-1867) 含 `SyncDir` failpoint + `dir_fsync_num`（成功后计数），不入
  fd_cache；访问器 :990。
- 测试 durability_dir.rs：懒建 root==2 / 已存在==1 / 失败上抛不计数的重试 +1
  （:39-72），目录真实存在断言 :53-54。

### perf-95 bench test_secs — 完善（重点核对项）

- `--test-secs`（main.rs:277，默认 `DEFAULT_TEST_SECS=60.0` :82，解析 .max(0.0)
  :346-350）；`window_duration` :738-744（0/负/NaN→ZERO，∞→MAX）。
- **时间窗语义对齐 C++**：点读窗口循环判停（:696-732，至少一样本）；scan/scan_page
  整轮重复至窗口耗尽（:874-883）；**写阶段固定次数** `while start < rows`
  (:796-799)——与 C++ `test_secs` 只 gate read/scan 一致。
- `PerfData.test_secs` :209 + 回兼容测试；doc/metrics.md §6 :129-141；三条新单测
  (:1207/:1240/:1252) 断言真实（0.6s 下界、NaN/∞ 边界、旧 JSON 回读）。

### rev-34 cloud reopen — 完善（重点核对项）

- 实现为下沉修法（非原计划的入口改道）：`CloudStore::read_manifest`
  (fkv_cloud:1946-1970) 本地命中也回源，相等返本地 / 落后 `switch_manifest` 覆写 /
  远端缺失 `drop_manifest`+None；远端护栏 = `latest_remote_manifest_bytes`
  按 branch 取 **max term**（:2170-2186）。`Store::reopen` main 分支
  (fkv_store:1765 → fkv_task_reopen:79-84) → `Request::Reopen`（fkv_shard:2412）与
  懒开（:910-911）自动继承；`install_external_snapshot`（fkv_store:1782）→
  `refresh_manifest`（fkv_cloud:1999）供 ReopenExternal（fkv_shard:2438-2448）。
- 测试 stale_manifest.rs 两条：stale 本地被远端替换 + 写回不晋升（:47-122）；
  远端缺失 ResourceMissing 不懒创建、恢复后可读（:127-172）。
- 残留：Standby 懒开无 sync_standby_partition 在盘属实（sync 仅 :2415/:2425/:2438
  三处）→ 新立 rev-46。

### rev-41 MemCachedPage 裁剪 — 完善（B 路径登记）

- migration.md §8.1（:238）四行登记（:248-251：pin / WaitingZone / swizzle /
  data page cache），每行含 C++ 出处、不需理由、重开条件。
- 无 fkv_cache crate；core 侧无 swizzle 写入路径，与登记一致。
- COMPAT_BASELINE 登记行欠账（DONE 自认）→ eng-93。

## 新立项

1. `issue/new/eng-93-compat-baseline-buffer-pool-deviation-line-and-stale-readme-path.md`
   （P3 文档）：rev-41 欠的 COMPAT_BASELINE 登记行（加行后必须重跑 gen_fixtures
   刷 38 份 meta.txt）+ fixtures/README.md:154 eng-91 过期路径（本次归档产生）。
2. `issue/new/rev-46-standby-lazy-open-skips-sync-standby-partition.md`
   （P2）：rev-34 带出的 StandbyReplica 懒开缺口，ensure_table_ready 直调
   store.reopen、无远端/主侧护栏，首访过期本地 manifest 窗口在盘核实。

## 备注

- 本组无「在途待下轮」：所有 mismatch 检查均为 0；热文件（03:48–03:55 mtime）
  的核对以内容锚点完成且全部命中。
- 归档后 `issue/new/00-index.md` 的 done/ 表与 `doc/progress.md` 计数需协调者
  例行刷新（含 new/ 由 0 → 2）。
