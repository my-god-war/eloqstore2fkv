# r6 收尾 lane 复审报告（2026-11-05 r6b）

范围：`issue/done/` 全部剩余 28 条（r6 五个主题 lane 复核约 186 条后跳过／归属交叉／
当时在途的条目）。方法：逐条按 md 内声称的实现要点与测试文件回读当前源码，抽读关键
断言（非仅文件名），证据落实到 file:line；只读源码，未运行 cargo/nextest，未改 crate/。

## 总结果

| 指标 | 数 |
|---|---|
| 复核 | 28 |
| 通过（已移入 reviewed/） | 28 |
| 在途（待下轮） | 0 |
| 声称不实 / 回归 / 测试缺失 | 0 |
| 新立项 | 1（eng-91） |

**判定：未达「完善」**——条目全部通过，但存在 1 条新立项
（`issue/new/eng-91-compat-baseline-evidence-line-stale-and-baked-into-fixtures.md`）。
按「完善 = 零新立项 + 全部通过或在途有主」的口径，eng-91 归档前保持「基本完善，余 1 项」。

注：任务预告的 12d / eng-34b / eng-39 / eng-32a 等「最近一两天完成」条目，全部声称与
代码一致，无并行会话半途状态；工作区虽大量 modified（git status），但不影响本批判定。
`27b` 系列不在本批（已在 reviewed/）。

## 逐条去向

| 条目 | 结论 | 关键证据（file:line，详见各 md 末「复审（2026-11-05 r6b）」） |
|---|---|---|
| `01-cpp-oracle-and-fixtures` | 通过 | corpus.rs:46-56/710/742/781、main.cpp:167/461/707、oracle.yml:117/140/146、fixtures 16+16 语料；遗留基线文案 → eng-91 |
| `12d-runtime-cloud-store-unified-budget` | 通过 | fkv_cloud lib.rs:392/504/510、backend.rs:44/119、runtime lib.rs:904/2128-2129、cloud.rs:250-313 |
| `32-stub-crates-missing-task-layer` | 通过 | 6 空壳 crate 目录已删、testkit lib.rs:27/66、map.rs:51/63 |
| `33-orphan-crates` | 通过 | tool/orphan.sh + allow.tsv（6 项）、ci.yml:29、fkv_gc lib.rs:7/55 |
| `33a-fkv-root-wiring` | 通过 | fkv_root 目录不存在、map.rs:29-30 改指 fkv_store |
| `core-22b-cloud-read-per-file-branch-term-key` | 通过 | fkv_io lib.rs:19、fkv_cloud:1745、fkv_store:1792、term_key.rs:71/116/163/203/274 |
| `core-23-file-gc-per-branch-inflight-guard` | 通过 | fkv_task_gc:24-27/66/112/225-266/481、inflight.rs 7 用例、gc_inflight.rs:12/34-38 |
| `eng-32a-read-batch-join-all` | 通过 | fkv_shard:59/702-720/1735/2194、read_batch.rs:110/164 |
| `eng-34-no-low-pq-yield-budget` | 通过 | fkv_shard:87/160/283-285/590/654、priority.rs:106/155、runtime:1152-1153 |
| `eng-34b-per-segment-maybe-yield` | 通过 | task_read:35/45/74/226、task_scan:125/148、task_gc:363/462/509、store:301/1516、shard:72/166/554、config:259、yield.rs:87/92/97 |
| `eng-39-reopen-no-driver-waiter` | 通过 | fkv_shard:913-946/949/971-988/572/632、auto_reopen.rs:115(147-157)/167/221 |
| `eng-60-create-branch-*` | 通过 | fkv_store:2818-2827（空分区短路）/2830/2937、branch_store.rs:305/327 |
| `eng-62-merge-*` | 通过 | fkv_btree:849 双游标流式归并，无 BTreeMap 中间层 |
| `eng-63-overflow-chain-*` | 通过（父条目收口） | fkv_io:55 read_page_li、task_read:255、fill(0) 已删 |
| `eng-63b-read-overflow-batch-read` | 通过 | task_read:226/255、read.rs:189/218（跨批链断言 256-260） |
| `eng-72-kv-options-*` | 通过 | fkv_config:123/145/164/55/259、fkv_types:32/37 |
| `eng-73-store-per-table-*` | 通过（父条目） | 见 eng-73a |
| `eng-73a-table-state-*` | 通过 | fkv_store:154/206/473/521-540、table_state_cleanup.rs:17/47/65 |
| `eng-75-large-value-*` | 通过 | fkv_store:2389-2476/2577-2578/2583-2684、large_value.rs:527/585 |
| `eng-76-manifest-snapshot-*` | 通过 | fkv_manifest:201/218、fkv_codec 复用 :8、store:1103 |
| `eng-77-cloud-cache-key-*` | 通过 | fkv_cloud:129/185/284/626/1803、cloud.rs:834 |
| `eng-82-unused-pub-scan-*` | 通过 | 5 符号全仓 0 定义、rsync.rs:22、fkv_standby:272/309/368 |
| `eng-83-unused-dependencies-*` | 通过 | 4 个 Cargo.toml 逐项 grep 0 命中；eng-83a 已 reviewed |
| `sto-57-kvoptions-*` | 通过（父条目） | 57a/57b/57c 见下 |
| `sto-57a-parse-size-*` | 通过 | fkv_config:573-596、config.rs:278 |
| `sto-57b-validate-*` | 通过 | fkv_config:60/316/404-430、config.rs:306/333 |
| `sto-57c-non-migrated-*` | 通过 | fkv_config:5-11 模块文档决策登记 |
| `sto-59-data-append-mode-*` | 通过 | fkv_store:73-77/510/1793、fkv_config:433-438、COMPAT_BASELINE:2/5-6、config.rs:346 |

## 新立项

- `issue/new/eng-91-compat-baseline-evidence-line-stale-and-baked-into-fixtures.md`（P3）
  - `COMPAT_BASELINE:10` 仍写「fixtures 无 payload；C++ oracle 未实现」，与 01 落地后的
    事实相反；`:12` 引用已失效的 `issue/todo/01-` 路径；且过期文案被 `fkv_ledger fixture`
    压进全部 38 份语料 `meta.txt` 的 `baseline=` 字段。01 的 md 两次声明该清理「不在本条
    lane」但无归口 issue，故由本 lane 立项。
  - 修法：更新证据块 + 重跑 `scripts/gen_fixtures.sh` 刷新 meta.txt。

## 复核中排除的候选（不立项理由）

- eng-77 声称 `tbl.clone()` 降到 3 处、实测 4 处：多出的 1 处系 12d CacheGroup 后续
  改动引入且属「交 owned 键」合法场合，非回归，已在 eng-77 复审小节记录，不立项。
- eng-34b 剩余限制（Gc/Compact 段内不可被前台插队、compact 循环未插桩、p99 基准）：
  md 内如实登记为限制而非完成项，且依赖 eng-32 ready set（todo/ 在案），不重复立项。
- 01 的 mapping/store oracle 字节实现：README「待补」明确登记，超出格式层 oracle 范围，
  不立项。

## 账面状态

- `issue/done/`：0 条（清空）。
- `issue/reviewed/`：213 条（含本轮 28 条 + 本报告）。
- `issue/new/`：`00-index.md` + `eng-91`。
- 下一步建议：eng-91 可与任何一次 `fkv_ledger sync` 同批处理；`00-index.md` 看板
  「已完成待复审」段落已过期（done 清空），待索引维护者更新。
