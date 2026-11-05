# r7 滚动复审 · 第 5 组报告（2026-11-05）

范围：`issue/done/` 五项。逐条按 DONE 声称核对源码与测试（关键断言抽读），
证据行号均亲读当前工作区。cargo/node 因并行会话占锁未运行，通过判定以
代码 + 测试文本核对为准；C++ 侧（`simple_bench.cpp`）亲读对照。

并行会话在途情况：复核期间 `fkv_shard/src/lib.rs`、`fkv_runtime/src/lib.rs`、
`fkv_bench/src/main.rs`、`eng-84`/`rev-02` done 文件均出现过小改（行号漂移、
DONE 增补验证记录），每次重读后核对——五条声称的内容全部在位且与最终读数
相符，无一属于「声称与代码不符」的在途冲突；并行会话另将 eng-32c / eng-90 /
rev-45 / rev-37 / rev-38 / rev-44 / rev-46 移入 done/（后续组范围，本组未动）。

## 结论一览

| 条目 | 判定 | 去向 |
|---|---|---|
| [`eng-33a`](../reviewed/eng-33a-write-gate-concurrent-dispatch.md) 写闸门并发派发 | 通过 | `reviewed/` |
| [`eng-84`](../reviewed/eng-84-global-api-unwired-and-list-archive-tags-missing.md) global API 残余 2/3 | 通过，全关闭 | `reviewed/` |
| [`rev-02`](../reviewed/rev-02-bench-write-workload-model-divergence.md) bench 写负载模型 | 通过 | `reviewed/` |
| [`rev-03`](../reviewed/rev-03-bench-scan-phase-no-concurrency-or-random-range.md) bench scan 并发/随机区间 | 通过 | `reviewed/` |
| [`rt-02`](../reviewed/rt-02-gc-mapping-doc-sync.md) Gc 映射文档同步 | 通过 | `reviewed/` |
| 新立项 [`rev-48`](../new/rev-48-write-concurrency-doc-lag-after-eng-33a.md) eng-33a 落地后文档滞后 | — | `new/` |

## eng-33a：通过

- **代码逐项属实**：`partition.rs` 全新（`PartitionStoreMap` + `Slot`，take/
  put_back/lose、root meta 近似 LRU 只驱逐空闲分区 `:311-350`）；`backend.rs:78-86`
  三变体 `Rc` + `derive(Clone)`；lib.rs `WriteTask/WriteDone/WriteFlight` +
  `spawn_put_group`（任务内 group commit、OOM 重试、逐条回执、写后 maintain）+
  `blocked_read_li`/`promote_blocked_reads` + 控制请求先 `write_flight.drain` +
  `run_due_reopen` 跳过在写分区；`fkv_store` 三方法在位（`adopt_attach:1825` /
  `with_index_cache_ledger:412` / `term:908`）。
- **测试抽读属实**：`write_concurrency.rs` 4 条断言亲读（峰值 ≥2 / =1 恒 1 /
  同分区 32 条 FIFO / 大写中他分区 get 先完成 + 两分区数据互不覆盖）。
- **与 20-write-batching / eng-32 交互（重点）**：合并在主循环串行段完成
  （`take_pending_while` + `merge_put_batch_li` + `prepare_put_group` 的
  admit/term 校验），执行在任务内独占 `&mut Store`——无并发合并竞争；同分区
  FIFO 由 `WriteGate::take_next` 跳过 `running` 分区保证；写派发前
  `finish_read_li` drain 全部在飞读后 `Rc::try_unwrap`（eng-32b 读 spawn 与
  写任务经 `blocked_read_li` 互斥，`Busy` 为防御路径）；`PutLarge` 不入组仍
  串行（`put_term` 只匹配 `Put`），与 DONE「剩余 1」一致。未发现正确性回归。
- **残余 → 新立项**：`06-gaps.md:10` / `02-execution-model.md:55-56` /
  `migration.md:206-207` 共 4 处仍写「跨分区写不并发 / 峰值写并发 1」，
  与落地相反且 eng-33a DONE 未自述（rt-02 范围外）→ `issue/new/rev-48`。

## eng-84：通过（本条全部关闭）

- **空目标 result name 属实**：`attach_ident_li`（`:962`）、最低 shard_id /
  回落 `(main,0)`（`:1694-1704`）、attach 成功才记录（`:1756-1757`）、
  `global_archive` 首回执名或 attach 身份名（`:1546-1562`）、delete 同路径
  回名（`:1591-1605`）、reopen/drop_table 无名保持空。DONE 增补的「已知
  近似」（C++ `branch_` Start 固定 vs fkv attach 身份）在案。
- **测试属实**：`empty_target_global_reply_uses_attach_identity`
  （archive_lifecycle.rs:209-258）六个场景断言逐条亲读。
- **per-table `ListArchiveTags` 在位**：api/shard/store/runtime 四层接线
  （含 `op_of:1256`、`dispatch_read:2329`，DONE 引 :1173/2192 为改前行号）+
  `per_table_ext.rs` 空清单 / `[(v1,1),(v2,1)]` 断言。
- **残余 1 复核**：`orphan_allow.tsv` 已无 `fkv_runtime` 行（eng-84a 已交付）。
  至此本条 1/2/3 全部闭环。

## rev-02：通过

- **batch 归属单分区**：`WriteLane::next_batch` 恰好 `len` 条 + 分区内回绕
  （`:788-798`）；`write_batch_is_per_partition_entries` 断言 4 分区 batch
  8192 单请求 8192 条且 key 属分区（`:1906-1925`）。
- **去 barrier**：`run_write_lane` 完成即补发（`:849-897`），驱动 `join_all`
  独立流水（`:1269-1275`）。
- **`--write-batchs` 全系统口径正确**：C++ `WriteLoop` `for (i < FLAGS_write_batchs)`
  按 dequeue 跨全部 writer 计数（`simple_bench.cpp:186-199` 亲读）；Rust
  `AtomicU64` 动态认领 + 运行时断言 `kv_num == N × batch_size`（`:1301-1305`）；
  `PerfData.write_batchs` serde(default)。
- **partitions=1 行为不变（重点）**：Once 模式每批 `remaining.min(batch_size)`
  与旧循环逐请求等价；运行时断言 `kv_num == rows` 且
  `batch_num == Σ ceil(count/batch_size)`（`:1292-1300`）钉死不变量；CI
  partitions 默认 1 不受影响。
- **delete 分区内回退 + 存活闭式**：`delete_target`/`is_live`/`live_key_num`/
  `live_after`（`:638-691`）+ `live_after_covers_partial_passes` 全分支。
  分片语义保留（甄别裁定）已在 `doc/metrics.md` §6.1 登记（`:156-174`）。
  声称的 7 个新测试全部在位并抽读。

## rev-03：通过

- **并发链**：`ScanLane` 链数 `partitions × read_per_part`、`id % partitions`
  轮转、独立种子（`:1021-1046`）+ `scan_lanes_match_cpp_scanner_num`。
- **随机区间公式与 C++ 逐字对齐（重点）**：亲读 `simple_bench.cpp:443-464`
  对照 `scan_range`/`scan_kvs`（`:1052-1073`）——0 扫尾、
  `max(1, bytes/val_size)`、`max_start` 边界（取不到本身）、`end = start +
  scan_kvs` 全等；`value_size = kv_size - 8` 两侧同口径。
- **页请求口径**：每请求一页 256（`:109`/`:1140-1143`）、链内时间窗 gate、
  指标拆分 `scan_*`（页）/ `fullscan_*`（附加，逐轮条数恒等断言 `:1391-1414`）、
  断代登记 `doc/metrics.md` §6.2（`:189-193`）。debug 逐 key 校验属分区 /
  区间 / `live_after` 存活（`:1153-1171`）。

## rt-02：通过

- 7 处逐条亲读属实（metrics.md §1 两行一对一 `:28-29`、四行补齐 `:40-44`、
  对账规则只剩 Drop `:54-59`、§3 21 个 Op `:79`、06-gaps 收敛化 `:35-36/:46+`、
  02-execution-model:27、migration.md:207 M4）。
- 独立验证：`op_enum!`（`fkv_metrics/src/lib.rs:112-133`）与 metrics.md §1
  表首列逐行全等（21/21，顺序一致）；`doc/` 残留 grep（「16 个 Op」「余
  FileGc」「合并为单条」「file_gc + local_gc」）零命中；`crate/` 无
  `Request::Gc`/`Op::Gc`。

## 新立项

- `issue/new/rev-48-write-concurrency-doc-lag-after-eng-33a.md`（P3，纯文档）：
  eng-33a 落地后 3 份文档 4 处「跨分区写不并发」陈述失实 + 2 个
  `issue/todo/eng-33a` 失效链接。去重核实：todo/new 无承接；06-gaps 同文件
  的 eng-90 / rev-45 行归各自复审组处理，本条只管写并发陈述。

## 目录状态

`done/` 本组 5 份已 mv 入 `reviewed/`（余 eng-32c / eng-90 / rev-45 / rev-37 /
rev-38 / rev-44 / rev-46 为并行会话新增，归后续组）；`new/` 增 rev-48。
`00-index.md` 快照（todo/done 计数与条目）已滞后，归看板维护方，未代改。
