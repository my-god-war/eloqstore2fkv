# eloqstore2fkv 差异审查看板

口径：审查意见逐条核对源码后分流。

- 已实现且有测试证据 → `issue/done/`，复审后移入 `issue/reviewed/`
- 已采纳、待开发 / 剩余验证 → `issue/todo/`
- 不采纳、附理由 → `issue/reject/`
- 复审后拆出的新任务 → `issue/new/`（本目录只放新意见；索引不计入条目数）

审查基线：`eloqstore 82b774c`；本地 compio 源码 `0.20.0-beta.1`；fkv 依赖 compio `0.19.2`。

## 当前快照（2026-11-05）

| 目录 | 条目数 | 说明 |
|---|---:|---|
| `new/` | 5 | 已复核为真缺口，待开发（不含本索引） |
| `todo/` | 0 | 无待开发条目 |
| `done/` | 9 | 已开发 + 测试证据，待复审后归档 |
| `reviewed/` | 295 | 已复审归档；含 `00-*.md` 主题报告 |
| `reject/` | 10 | 不采纳或已失效 |

## 当前仍缺什么

`new/` 现有 5 条，均为复核后的真缺口；其中 3 条只剩 C++ / CI 半侧：

- [`gap-02`](../new/gap-02-engine-level-v2-v4-cross-read.md)：
  Rust 打开 C++ 产出的 **store 目录**、Rust 续写 → C++ 读回仍为 0。格式层（含
  `mapping` / `store` 信封）已双向验证，但不构成引擎级结论，`L1` 升级前置未变。
  **需在 4 KiB Linux 构建 eloqstore，本机做不了。**
- [`gap-03`](../new/gap-03-v3-op-log-differential.md)：
  Rust 半侧已落地（文本 op-log + 确定性 dump + 6 条场景）；C++ 半侧的 dump 差分未做，
  与 gap-02 同一环境前置。
- [`gap-04`](../new/gap-04-v5-crash-fuzz-closure.md)：
  真后端 intra-op 闭环与撕裂尾物化已落地（见 reviewed/gap-10、`fkv_store/tests/`
  的 `crash_failpoint_invariants.rs` / `torn_manifest.rs`）；「随机 kill point」与
  「失败输出含 `seed` / `op_index`」未做。
- [`gap-05`](../new/gap-05-v6-perf-baseline-asset.md)：
  `perf.yml` 门槛机制已落地（缺资产即 `exit 78` 变红、含 4 KiB 断言）；
  **基线资产仍未产出**，需在 4 KiB runner 上跑一次
  `workflow_dispatch + publish_baseline=true`。
- [`gap-13`](../new/gap-13-torn-padding-defence-deleted-in-unrelated-commit.md)：
  回归已由上游 `0ba0ed5` 修复；剩余只有 husky `pre-commit` 的 `|| true`（一行，
  且属并发会话正在改的 workflows 区域）。

本轮回审后归档（实现均已落地，详见各文件「复审归档」节）：
`gap-01`（账本 543 条分流；`scaffold` 判定低价值未做）、`gap-06`（请求 deadline 配置面）、
`gap-07`（`fkv_io_sim` 接线）、`gap-08`（语料静默跳过）。

已登记的范围外 / 有意差异（不重复列 issue）：

- 单进程多 DS 托管、`GlobalRegisteredMemory` 段 slab、data page cache / buffer pool、
  branch file id 预留 1024、TTL 读过滤默认、bench `key_interval` 网格模型、
  性能/资源 inert 旋钮、SDK 单根限制 —— 见
  [`COMPAT_BASELINE`](../../COMPAT_BASELINE) 与
  [`06-gaps.md`](../../../fkv/doc/architecture/06-gaps.md)。

## 已完成待复审（done，9）

| 文件 | 结论 |
|---|---|
| [`gap-09`](../done/gap-09-mapping-store-fixtures-fkv-envelope-not-engine-interop.md) | 级别按 `kind` 派生（不新增冗余字段），README 写明口径；引擎级仍 0 条挂在 gap-02 |
| [`gap-10`](../done/gap-10-crash-fuzz-op-boundary-only-no-torn-write.md) | 崩溃闭环打到真后端：intra-op kill → 重开必须等于某个 op 前缀；撕裂尾盘上物化（截断/垃圾尾/损坏） |
| [`gap-11`](../done/gap-11-failpoint-name-sets-diverged-between-backends.md) | 名字收敛到 `fkv_failpoint::points` 常量；`fail_point!`/`kill_point!` 只收常量标识符 → 漂移改为编译失败 |
| [`gap-12`](../done/gap-12-compio-kill-point-placement-gaps.md) | compio 注入点名字↔窗口一一对应：`:Rename` 移到 `rename` 之后，补 `WriteFile:Rename` 与 `SyncDir` kill 点，`reset_manifest` 截断契约入文档 |
| [`r10b-01`](../done/r10b-01-open-table-li-dead-pub-accessor-misleading-doc.md) | `open_table_li` 文档改为测试/诊断接缝 |
| [`v-01`](../done/v-01-fkv-config-overflow-pointers-validate-boundary-untested.md) | `overflow_pointers` 0/MAX/MAX+1 校验测试 |
| [`v-05`](../done/v-05-open-error-errno-classification-tests-missing.md) | data/segment 打开 errno 分类测试 |
| [`v-06`](../done/v-06-write-file-tmp-fsync-no-failpoint-no-durability-test.md) | `write_file` tmp fdatasync 失败注入测试 |
| [`v-09`](../done/v-09-rev46-standby-lazy-open-divergence-unregistered.md) | standby 懒开先追 master 登记基线 |

## 已复审（reviewed）

295 条已归档；主题报告按轮次：

- `00-r6-*.md`（6 份）
- `00-r7-group1.md` … `00-r7-group5.md`
- `00-v-lane-{a,b,c,d}.md`
- [`00-audit-2026-11-05.md`](../reviewed/00-audit-2026-11-05.md)

## 已拒绝（reject，10）

| 文件 | 理由摘要 |
|---|---|
| [`03`](../reject/03-shard-request-concurrency.md) | 主体与 eng-32 重复；验收 / 指标由 32c 承接 |
| [`fmt-16`](../reject/fmt-16-cpp-stub-crates-empty.md) | `_cpp` 空壳是故意的差分占位，不按普通 crate 补实现 |
| [`fmt-20b`](../reject/fmt-20b-page-builder-reset-reuse.md) | C++ `Reset()` 复用换来每页 memcpy；Rust 零拷贝移交更优 |
| [`perf-83`](../reject/perf-83-parking-lot-in-fkv-object-cloud-layer.md) | 云层锁在生命周期 / Attach 边界，不属热路径 |
| [`rev-33`](../reject/rev-33-put-group-commit-skips-expired-marking.md) | 意见基于旧快照；当前 group commit 已标 `Expired` |
| [`rev-35`](../reject/rev-35-kill-point-injection-gaps.md) | 两个注入点均等效覆盖，无独立缺口 |
| [`rev-40`](../reject/rev-40-partition-filter-not-implemented.md) | 合并进 sto-61b；多 DS 托管按宿主集成范围外关闭 |
| [`rev-43`](../reject/rev-43-btree-split-rebalance-leaf-chain-not-in-core.md) | split / 叶链层次是已决策契约，非缺陷 |
| [`rev-47`](../reject/rev-47-space-stats-still-id-level-after-variant-gc.md) | 统计 id 级与 C++ 同口径非偏离 |
| [`sto-52`](../reject/sto-52-replayer-getmapper-allocator-reconstruction-divergence.md) | `GetMapper` 重建差异按范围排除，不纳入互操作保证 |

## 历史

旧版索引按轮次堆叠叙事；历史证据看 `reviewed/00-*.md` 与各 issue 文件末尾的
`复审` / `DONE` 小节。
