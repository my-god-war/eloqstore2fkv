# r6 复审报告：io / compio / bench / ledger lane

日期：2026-11-05。范围：`crate/io/{fkv_io,fkv_io_compio,fkv_io_mem,fkv_io_sim,
fkv_object,fkv_object_s3}`、`crate/perf/fkv_bench`、`tool/fkv_ledger`（含
oracle/fixture 管线、perf CI/看板），对照 eloqstore 82b774c 与 compio 0.19.2
（本地源码 /Users/z/git/db/kv/compio）。

## 任务 A：done 复核（49 条，49 通过 / 0 未过）

每条均已按 md 内声称的实现要点与测试文件到当前源码逐项核对（实现存在、
测试文件存在、抽读关键断言），证据行号写入各文件末尾
「## 复审（2026-11-05 r6）」小节，然后 mv 到 `issue/reviewed/`。

### IO / compio 组（24 条）

| 条目 | 结论 | 关键证据（r6 小节内有全文） |
|---|---|---|
| io-03 | 通过 | write_page_group 写池复用 + write_page.rs 重复批次 alloc 不增长；task_read/write/scan fill(0) 零残留 |
| io-21 | 通过 | is_direct_path 匹配 segment_、direct_segment、read_segment 对齐读、DIRECT_POOL_BUDGET、hardening.rs 两个 roundtrip（linux-gated） |
| io-26 | 通过 | DEFAULT_IO_QUEUE_SIZE=4096 + build_runtime 四参数 + 回落；FdCache limit==0→usize::MAX + 测试 |
| io-27 | 通过 | WriteGate/FixedWritePool/take_fixed_slot/write_page_group/失败先记脏；MAX_INFLIGHT_WRITE=16 已被 io-40 正式演进为 32768 接配置 |
| io-29/29a/29b/29c/29d | 通过 | ListPage 契约、list_with_offset 下推 start-after（mock 断言）、cloud 分页 API（符号演进为 FilePage/RemoteFileEntry，功能等价）、prewarm unfold 流水线 |
| io-32 | 通过 | FdGuard=SharedFd<File> 三处 to_shared_fd + guard 生命周期测试 |
| io-34 | 通过 | 全链 Bytes（trait/actor/Mem/put_range freeze），两 crate to_vec() 零命中 |
| io-35 | 通过 | managed_segment_read_supported + read_managed_at 带 buf.len()>=len 采用 + 回落 + shard 池配置 + 两个测试 |
| io-36 | 通过 | DEFAULT_MAX_WRITE_BATCH_PAGES=32 + 连续 run 切分 + config→runtime→shard→SDK 接线 + batch.rs 切分测试 |
| io-37 | 通过 | SlotState{buf,inflight,released} + SlotLease 完成驱动归还 + 三处提交点租约 + 单测/集成测试（含变异验证记录） |
| io-38 | 通过 | write_segment_fixed 提交前对齐护栏→InvalidArgs（非静默回落）+ 两个测试 |
| io-39 | 通过 | read_len==0→saw_eof 不重试、耗尽 EndOfFile/Corrupted 分流；注：DONE 措辞「不再尝试旧 term 候选」与实现不符，实现保留候选遍历（防新 term 稀疏文件遮蔽），语义更优，判描述瑕疵 |
| io-40 | 通过 | 旧双常量已删、单值 max_inflight_write 贯通五层、FIXED_WRITE_SLOT_LIMIT=16 语义注释、write_gate 单测 |
| io-41 | 通过 | fkv_io 契约文档三件套 + compio 短读零填充/错误分支归还 + mem 排序 + 三份镜像 contract.rs |
| io-43 | 通过 | forget_table_state 清 7 项状态 + evict.rs「截短 manifest 后 append 走 fstat」断言 |
| eng-63a | 通过 | PageIo::read_page_li 默认串行 + compio try_join_all 覆盖 + batch.rs 测试 |
| eng-74/74a/74b/74c | 通过 | manifest 追加/原子写、FileKind 收敛、ReadChunk/write_chunk 统一，符号与 failpoint 全部在位 |
| eng-79/79a/79b | 通过 | O(1) LRU（FdNode 槽位数组+双向链+双索引）、fd_limit 全链接线（config→runtime 均分→shard/backend→SDK）、压力测试 |

### 验证工具层（4 条）

- `01-cpp-oracle-and-fixtures`：通过。cpp/oracle（main.cpp/standalone/build.sh）、
  scripts/fetch_eloqstore.sh、fixtures/cpp 4 类 15 个 payload 全非空、
  fkv_ledger SCHEMA_VERSION/MATRIX/ORACLE_KINDS、三个 fkv_* tests/cpp_oracle.rs、
  oracle.yml fixture-diff 作业全部在位；未完成项（反向 fixture、首差异 offset、
  mapping/store 字节）md 已如实列明。
- `32a`（fkv_io_sim）：通过。SimStore 双层 + crash/restart/snapshot + 5 组测试
  + orphan test-only 白名单。
- `32b`（TempRoot 迁移）：通过。lane 内 3 crate 已迁；全仓 fn temp_root( 残留
  恰为 fkv_runtime(22)/fkv_shard(13)，与「保留待 owner」记录一致。
- `33b`（orphan guard CI）：通过。ci.yml clippy+orphan.sh、owner 归档检查
  （ARCHIVE_DIR_LI）；白名单 7→6 项（fkv_standby 已接线后移除）属正常演进。

### bench / perf 组（21 条，含 4 条拆分总纲）

perf-71/72/72a/73/74/75/75a/75b/76/77/77a/78/78a/79/80/81/81a/82a 全部通过。
关键抽验：随机 key + 读前 reopen（71）、LatencyHist 与 *_us 指标（72a）、
ReadLane 并发链 + join_all（73）、key_interval/del_ratio/workload 谓词（74）、
perf.yml env 唯一来源 + fetch cpp baseline + ratio gate/enforce（75a/75b）、
gate.mjs 策略表与 env 覆盖（75b）、web/src/lib/perf.js + perf.test.js（76）、
coarsetime 时间戳 + Instant 窗口注释（77a）、scan 零 dyn + pin!（78a）、
排序引用去重（79）、VmHWM rss_peak_kb（80）、sdk exists/put_at（81）、
KvEntry/scan_entry 族（81a）、PendingWrite 1MiB 预算（82a）。
perf-72/75/77/78 四条为拆分记录，子项落地均已独立核实。

## 任务 B：新立项（1 条）

- **`issue/new/perf-95-bench-fixed-iterations-no-test-secs-window.md`（P2）**：
  bench 各阶段按固定次数计量、无 `--test-secs` 时间窗口；C++ `simple_bench`
  按 `test_secs=60` 稳态窗口采样延迟分位（`simple_bench.cpp:31/:336/:627`）。
  归属链断裂：perf-72 明确把该项推给 perf-74，perf-74 审查结论与 DONE 均未接，
  全 issue 目录 grep `test_secs` 无实际覆盖 → 真空缺口。影响 p99 ≤ 1.2× gate
  两边窗口口径不可比。非热点文件（fkv_bench 归本 lane）。

其余侦查点均判定不立项（理由）：
- `write_page_li` 组 future 在 WriteGate 许可内做 `open_write_file`（许可窗口
  含 open）：fd 缓存命中路径零系统调用，P3 以下，不立项。
- `set_file_identity` 不清 manifest_offset/dirty：路径按 branch/term 命名隔离、
  manifest 替换走 forget_fd，良性。
- `sync_data` 的 FSYNC_INFLIGHT_LIMIT=16 独立常量：fdatasync 与写组是两种资源，
  与 io-40「单一在飞写上限」不冲突。
- checksum 失败落旧 term 候选（io-31 议题）：`issue/new/00-index.md:134/173`
  已有记录（第二轮 P1 表格 + 建议顺序），非新发现，不重复立项，列遗留风险。

## 遗留风险

1. **io-31 议题悬空**：`read_page` checksum 失败/短读后仍会尝试旧 term 候选，
   旧候选 checksum 通过即返回旧 term 页（静默陈旧窗口）。00-index 有记录但
   issue 目录无实体文件、无 owner——建议 coordinator 补立实体或显式归档决议。
2. **perf gate 单边**：`perf-cpp-<arch>.json` release asset 生产侧（oracle 环境
   跑 simple_bench）仍缺位，ratio gate 当前只有 rust 自回归兜底
   （perf-75a/75b 遗留，01 剩余范围）。
3. io-21 遗留：manifest O_DIRECT 需先统一 framing 补零/截断契约（md 记录在案）。
4. io-26 遗留：fixed file 明确跳过；DEFER_TASKRUN p99 收益需 Linux 真机压测。
5. io-40 遗留：FIXED_WRITE_SLOT_LIMIT=16 固定，配置放大时槽位缓存不放大。
6. perf-77a 遗留：bench_gxhash 循环内每轮读时钟。
7. **行号快照**：另一会话正在开发 issue/todo 12 项（热改 fkv_io/fkv_io_compio），
   本报告行号为 2026-11-05 快照，后续以各 md r6 小节为准。
8. macOS 复审环境下 linux-gated 测试（O_DIRECT/fixed 路径）无法本机执行，
   以 md 内 Linux 容器实跑记录 + 交叉编译 clippy 为凭（io-21/27/32/37/38）。

## 跳过项（不归本 lane，未复核）

done/ 中 `sto-57*`、`eng-62/63b/72/73/73a/75/76/82/83`、`core-22b/23`、
`12d/32/33a`、`eng-32a/34/39/60` 及并行会话新移入的 `cld-*`、`rev-*` 等，
落点在 store/engine/runtime/recovery lane。`eng-63b`（overflow 批读）落
`fkv_task_read`，归 read lane。

## 最终判定：不完善

缺口清单：

1. 新立项 perf-95（bench 无 test_secs 时间窗口，p99 验收口径失真）——修法与
   验收已写入 issue。
2. 已知未覆盖差异 io-31（旧 term 候选回退窗口）无实体 issue 跟踪。

lane 内 done 条目 49/49 复核通过；除上述两点外未发现新的正确性/性能差异。
