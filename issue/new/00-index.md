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
| `new/` | 9 | 已复核为真缺口，待开发（不含本索引） |
| `todo/` | 0 | 无待开发条目 |
| `done/` | 9 | 已开发 + 测试证据，待复审后归档 |
| `reviewed/` | 291 | 已复审归档；含 `00-*.md` 主题报告 |
| `reject/` | 10 | 不采纳或已失效 |

## 当前仍缺什么

原索引把 6 条 gap 移入 `reviewed/` 并声称“均为验证 / 工具链债务”不成立。
2026-11-05 复核已重新打开，`new/` 现有 8 条真缺口：

- [`gap-01`](../new/gap-01-ledger-todo-vs-scope-out-reconciliation.md)：
  账本 543 todo 未分流；`map` / `ignore` 工具已实现但从未运行。
- [`gap-02`](../new/gap-02-engine-level-v2-v4-cross-read.md)：
  `mapping` / `store` oracle 直接 exit 3，6 个占位语料；引擎级 V2/V4 未执行。
- [`gap-03`](../new/gap-03-v3-op-log-differential.md)：
  `OpRecord` 无 payload、无序列化、无 replay driver。
- [`gap-04`](../new/gap-04-v5-crash-fuzz-closure.md)：
  无 kill point 枚举闭环，`kill_point` 的 abort 从未被触发。
- [`gap-05`](../new/gap-05-v6-perf-baseline-asset.md)：
  两个仓都没有 `perf.yml`，没有任何可阻断的性能门槛或基线资产。
- [`gap-06`](../new/gap-06-request-deadline-config-unreachable.md)：
  `KvOption` 无 timeout 字段，`start_runtime` 不接线，功能缺口。
- [`gap-07`](../new/gap-07-fkv-io-sim-unwired.md)：
  `fkv_io_sim` 是孤立 crate，引擎级 crash / replay 接不起来。
- [`gap-08`](../new/gap-08-fixture-tests-silent-skip.md)：
  oracle 语料缺失时测试静默 pass，CI 绿灯不能证明 oracle 跑过。
- [`gap-11`](../new/gap-11-failpoint-name-sets-diverged-between-backends.md)：
  compio / sim 的 failpoint 名字集合已发散，未命中会静默 no-op。
- [`gap-13`](../new/gap-13-torn-padding-defence-deleted-in-unrelated-commit.md)：
  撕裂 padding 防线曾被无关 commit 删除并红着落地；剩余动作是让 husky 的 fmt
  检查变成阻断式（当前 `|| true`），CI 本身已覆盖全工作区。

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

291 条已归档；主题报告按轮次：

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
