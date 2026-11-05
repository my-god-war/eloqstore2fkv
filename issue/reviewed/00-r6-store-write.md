# r6 复审报告：store+write 写路径 lane

日期：2026-11-05。Lane crate：`fkv_store` / `fkv_task_write` / `fkv_task_compact` /
`fkv_api` / `fkv_metrics`（另涉写路径载体 `fkv_shard` / `fkv_btree` / `fkv_mapping` /
`fkv_task_reopen` 的条目按内容归属一并复核）。

## 一、done 复核：24 条全部通过

每条已在其 md 末尾追加「## 复审（2026-11-05 r6）」并 mv 到 `issue/reviewed/`，
证据均为本轮亲读源码（file:line + 关键断言抽读）：

| 条目 | 要点 |
|---|---|
| eng-33 | WriteGate per-partition FIFO + `max_write_concurrency` 接线（write_gate.rs:37-208，5 单测） |
| eng-33b | `write_queue_depth`/`write_running` gauge（fkv_metrics:282-290 + publish 六点） |
| 20-write-batching | group commit（shard lib:1575 合并、merge_put_batches 语义、write_group_commit.rs 256 put fsync<128） |
| sto-65 | 旧 ts delete 守卫（fkv_task_write:695）+ 回归测试 write.rs:1272 |
| sto-66 | 同 expire 覆盖守卫（:709-719）+ 回归测试 :1377 |
| rev-01a | put_at 双 mapper 批（fkv_store:1319-1345）+ failpoint 测试 maintenance_batch.rs:136 |
| core-25 | compact/clean_expired/truncate 提交后移（:2360/:2195/:1908）+ 3 failpoint 测试 |
| eng-53 | 字节中点切分 pick_split（:1252）+ overflow slot 迁移 + 空表首写重平衡，4 测试 |
| eng-59 | archive_tag 20 位微秒（fkv_api:12）+ 列盘归档 + KvOption 接线，3 测试 |
| eng-60b | segment 预压缩经 sto-56b 承接落地（store:2939-2944）；DONE 文本「未实现」已过时（见遗留） |
| eng-60d | create_branch 空分区短路（:2795-2805）+ data 预压缩，2 测试 |
| sto-56b | segment compaction 全链（task_compact:124、gc plan、sdk 透传），2 集成测试 |
| eng-62a | 流式 merge（fkv_btree:522-884 双指针归并），3 测试 |
| eng-68a | BatchJournal.changed_len 截断（fkv_mapping:560/:723）+ 测试 |
| eng-68b | compact/truncate 出口清 changed；`assert_changed_consumed` 已被 27b 工作内联为 debug_assert（:668-682），不变量仍在 |
| sto-56c | 判定「主张过时仅补测试」成立；needs_maintain O(1)（:2978-2996）+ gc_store.rs:236 |
| eng-34c | 调度三指标 + 打点 + priority.rs:179（含 p99 夹具缓做的合理理由） |
| eng-39a/b/c | reopen driver/waiter + Cloud 模式挂等重排（shard:900-973/:552），5 测试；死代码确删 |
| perf-79 | 入口去重零 clone（fkv_task_write:217-232） |
| perf-82a | PendingWrite 跨组聚合（:73/:81-123）+ cross_group_writes_flush_once |
| sto-60 | Op↔request_type 映射 API + 19 标签测试 + gauge 拆分 |
| sto-67 | 索引页 1/4 重分配（:1570-1645，region 直拷）+ 2 测试，断链闭环 |

**未过：0 条。**

## 二、跳过项（并行会话移动 / 非 lane）

- 复审期间被并行会话移走：sto-62/62a/62b/62c、rev-02、perf-84/85/86/87、eng-85、core-27。
- 按 lane 分工跳过（读/扫/GC 侧）：eng-64/65/66*、eng-63a/63b（overflow 链读）、io-36/74c（io lane）。
- 归属存疑未复核（建议由结构/大值 lane 认领）：eng-75（put/get_large 同构去重）、
  eng-76/76b（manifest snapshot 摊平/零拷贝）。

## 三、新立项（2 条）

1. **`issue/new/eng-89-compact-missing-data-fsync-before-manifest.md`（P1，热点文件）**：
   `compact_file_li`（:2384）与 `compact_segment_file_li`（:3207）在 `append_mapping_log`
   前无 `io.sync_data`，而 put（:1497）/clean_expired（:2226）都有；C++ `UpdateMeta`
   （write_task.cpp:854-868）在 FlushManifest 前统一 SyncData，compact 亦走此路径
   （background_write.cpp:157）。崩溃窗口 + maintain 随后 gc 删旧文件 → 存活页可永久损坏。
2. **`issue/new/eng-90-merge-put-batches-per-entry-clone.md`（P3，热点文件）**：
   group commit 的 `merge_put_batches`（shard:1634-1654）每 entry 双 clone + 双 BTreeMap，
   重蹈 perf-79 已修模式；按 perf-79 引用折叠法改造。

## 四、遗留风险

1. eng-60b：`create_branch → segment 预压缩` exact 路径无直测（segment_compact.rs 只测
   维护路径与 branch 保留，branch_store.rs:299 只测 data 侧）——测试缺口，非功能缺口。
2. eng-39c：真实 S3 端到端未本地验证（md 已记录，归 12* 后续）。
3. `fkv_store/src/lib.rs` 为并行会话热点，本轮行号在会话内多次漂移（3384→3406 行），
   复审行号以内容定位为准。
4. **对他 lane 的提醒**：`issue/new/rev-33` 引用的 `run_put_group` 缺
   `state.lifecycle = Expired` 与当前源码不符——我实读（shard lib:1685-1688 一带）该标记
   **已存在**（含「与单条 Put 路径对齐」注释），rev-33 可能已过时，建议其作者复核后关闭。

## 五、判定

**不完善。**

- done 复核：24/24 通过（满足）。
- 新立项：2 条（不满足「零新立项」）：eng-89（P1，崩溃耐久性差异，C++ 有明确对照）、
  eng-90（P3，性能回退）。
- 缺口清单：eng-89 必修（sto-56b 系列落地后 compact 已是常规维护路径，窗口常态化）；
  eng-90 随 group commit owner 顺手修。
