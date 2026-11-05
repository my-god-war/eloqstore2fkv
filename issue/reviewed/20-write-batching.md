# 20 P1：没有 write buffering / group commit

## 现象

当前每次 `put`：

1. 写 data page（可能多个）
2. 写 TTL page
3. append manifest log
4. `sync_data` manifest 文件

`fkv_shard` 一次只处理一个请求，因此并发写入无法合并；每个 put 都独立触发 page IO + manifest fsync。

## C++ 对照

eloqstore 有 `write_buffer_aggregator` 和 append mode：

- `write_buffer_size` / `write_buffer_ratio` 控制写缓冲
- 并发写可以合并成更大的 page/file 写入
- manifest 提交和 data flush 有独立的批处理节奏

## 影响

- 小写入吞吐受 fsync 次数限制
- 每次 put 都产生 manifest log 记录，放大 manifest 体积
- 与 03（shard 串行）叠加，无法通过并发摊薄 fsync 成本

## 建议

1. 按 table 增加 write aggregator：
   - 收若干 queued put
   - 合并为一次 `write_batch`
   - 一次 manifest append / 一次 fsync
2. 引入 group commit：多个请求共享一次 manifest sync，完成后统一唤醒。
3. 保持单 table 写顺序与 timestamp/term 过滤语义。
4. 将平均 batch size、fsync 次数、写放大纳入 metrics。

## 验收

- 1000 个单 key put 的 manifest fsync 次数明显小于 1000
- 吞吐随并发写入上升
- 崩溃后已确认的 put 不丢失

## 审查结论（2026-11-04）

### 总结论

**部分采纳，移入 `issue/todo/`；但必须缩范围、改口径。**

- “每次 put 两段 fsync、无聚合提交”成立。
- “shard 一次一个请求”成立，但这属于 `eng-32` / `03` 的问题，本文只是引用它，不构成新发现。
- “按 table 增加 write aggregator”与 `eng-33` 的每表写队列重复，本文不应再定义队列。
- C++ 对照**部分不成立**：`WriteBufferAggregator` 是**单次 WriteTask 内**把连续页合并成一次提交，不是跨请求聚合，更不是 group commit；C++ 同样每条写请求各做一次 `SyncData` + `FlushManifest`。
- 真正无人覆盖、且 C++ 也没有的能力是**跨 put 共享 manifest fsync（group commit）**。它是超出兼容基线的增强项，不是盘面差异。
- 优先级建议 **P1 → P2**。零代码的最大收益先拿到：调用方把多 key 放进一个 `put(batch)` 即已合并为一次提交。

### 逐条核验表

| # | 原文断言 | 源码证据 | 判定 |
|---|---|---|---|
| 1 | 每次 put 写 data page（可能多个） | `fkv_store/src/lib.rs:900-917` 调 `write_batch`；`fkv_task_write/src/lib.rs:551,716,929,954` 落页 | 成立 |
| 2 | 每次 put 写 TTL page | `fkv_store/src/lib.rs:919-939`：仅 `ttl_li` 非空时写 TTL 树 | **部分成立**（有 expire/删 TTL 项才写） |
| 3 | 每次 put append manifest log | `fkv_store/src/lib.rs:999-1000` → `append_mapping_log`；首条走 `write_manifest_record`（`:965-998`），超 `manifest_limit` 改写 snapshot（`:1050-1078`） | 成立 |
| 4 | append 后 `sync_data` manifest | `fkv_io_compio/src/lib.rs:1159` `file.sync_data()`；数据文件另由 `fkv_store/src/lib.rs:962` `io.sync_data(tbl)` 同步（实现见 `fkv_io_compio/src/lib.rs:1094-1123`） | 成立；但顺序是 data fsync → manifest sync，原文“sync_data manifest 文件”表述混淆 |
| 5 | `fkv_shard` 一次只处理一个请求 | `fkv_shard/src/lib.rs:177` `while let ... rx.recv()`，`:178-185` await 完 `handle` 再取下一个；Put 分支 `:371-379` | 成立 |
| 6 | 并发写无法合并 | 同上；全仓无 `WriteGate`/`Semaphore`/aggregator/后台提交任务 | 成立 |
| 7 | 每次 put 一条 manifest 记录，放大体积 | `append_mapping_log` 每 put 一条；`manifest_limit` 会转 snapshot 抵消无界增长 | 成立，但“无界放大”不成立 |
| 8 | eloqstore 有 `write_buffer_aggregator` | `eloqstore/include/tasks/write_buffer_aggregator.h:24-72`；`src/tasks/write_task.cpp:329-400` 使用 | 成立 |
| 9 | 并发写可合并成更大 page/file 写入 | C++ 每分区同一时刻仅一个写任务（`include/storage/shard.h:300-333`，`src/storage/shard.cpp:623-656`）；`SetBatch(write_req->batch_)`（`shard.cpp:1098-1104`）只用一个请求的 batch；aggregator 只在**该请求内部**合并连续页 | **不成立**（无跨请求合并） |
| 10 | `write_buffer_size` / `write_buffer_ratio` 控制写缓冲 | `src/async_io_manager.cpp:301-345` 按 ratio 切分注册缓冲池；`write_task.cpp:188-194` 以 `write_buffer_size` 建 aggregator 缓冲 | 成立，但这是**预分配写缓冲池**，非 group commit |
| 11 | manifest 提交与 data flush 有独立批处理节奏 | `src/tasks/write_task.cpp:855-868`：每写任务固定 `WaitWrite → SyncData → WaitPendingUploads → FlushManifest` | **不成立**（无跨请求批节奏） |
| 12 | Rust 无 write aggregator / group commit | 仅 `write_page_li`（`fkv_io_compio/src/lib.rs:1043-1091`）对**同批连续页**合并一次 `write_at`；无跨请求层 | 成立 |
| 13 | `write_buffer_size` / `write_buffer_ratio` 已具备 | `fkv_config/src/lib.rs:46,48` 解析，`:208-213` 校验；`crate/engine`、`crate/io`、`crate/core` 无消费方 | 成立（配置悬空，未接线） |
| 14 | 验收：1000 个单 key put 的 fsync 明显小于 1000 | 现状 2 次 fsync/put；调用方若一次传 1000 entry 则已约 3 次 fsync | 成立，但须区分“单 key 单请求”与“批量请求”两种负载 |
| 15 | 崩溃后已确认 put 不丢失 | 当前顺序 `data fsync → manifest append+sync → 返回`，满足 | 成立；group commit 要保留该不变量 |

### 与既有 issue 的重叠

| issue | 关系 | 处理 |
|---|---|---|
| `eng-32-workloop-serialization.md` / `03-shard-request-concurrency.md` | **前置依赖**。串行 loop 是“无法合并并发写”的直接原因；并发化后 group commit 才需要真正的提交协调器 | 本文只引用，不重复设计 |
| `eng-33-write-gate-missing.md` | **直接重叠**。每表 FIFO 写队列 + `running_` 门与本文“按 table 增加 write aggregator”是同一块结构 | 队列归 `eng-33`；本文只负责“提交合并” |
| `io-03-compio-buffer-double-copy-unaligned-pool-and-vectored-write.md` | C++ aggregator 的底座是预注册写缓冲池与批量提交；Rust 对应缺口在 `io-03` | 缓冲池归 `io-03` |
| `io-06-segment-write-...` | 写系统调用次数、预分配，属段路径 | 独立 |
| `eng-49-write-batch-per-group-descent-...` | 批内索引页写放大 | 独立，且收益更确定 |
| `core-16-write-batch-dedup-last-wins-ignores-timestamp.md` | 同批去重语义 | 独立 |

结论：`20` 的独有价值只剩 **group commit + 提交层指标**；其余部分分别被 `eng-32/03`、`eng-33`、`io-03` 覆盖。

### 采纳后的开发计划

阶段 0：零代码先拿收益（1 天）

- 文档明确：单次 `put(tbl, batch)` 已把多 key 合并为一次 data fsync + 一次 manifest sync；客户端应优先批量，而不是 N 次单 key 请求。
- 验收基线：分别测 1000 条单 key 单请求、1000 条单次批量请求的 `fsync_num` / 吞吐，写入 `tests/` 基准，避免后续优化对着错误基线。

阶段 1：拆分提交边界（2-3 天，低风险）

- 文件 `crate/engine/fkv_store/src/lib.rs`
  - 把 `put_at_inner` 拆成 `prepare_write`（写页、更新 `root_map`/`ttl_root_map` 候选、保留回滚上下文）与 `commit_write`（`io.sync_data` + `append_mapping_log`）。
  - 保持 `begin_batch` / `commit_batch` / `abort_batch` 每请求语义不变；只有 IO 提交被延迟。
  - 提交失败时把组内所有候选 root 回滚到组前，并让整组请求返回错误（不能部分成功）。
- 该阶段不引入新队列，不改变调用方语义。

阶段 2：group commit（3-5 天，依赖阶段 1）

- 在 shard 请求循环内做**连续写聚合**，不新建第二套队列：
  - 从 `rx` 收第一条写请求后，用 `try_recv` 继续吸收同一 table、同一 term、非 barrier 的写请求，受 `max_group_puts`、`max_group_bytes`、`max_group_delay_us` 约束。
  - 对同一 table 的组，`prepare` 全部后执行一次 `io.sync_data(tbl)`；manifest 只追加一条合并记录：`dirty_page_li` 已是累计集合，root/ttl_root 取组内最后一次，segment delta 同理合并。
  - 非写请求（Get/Scan/Attach/Detach/Shutdown）作为 barrier：先提交当前组再处理，保证读不看到未提交状态。
  - 组提交成功后再统一回包。
- 若先落地 `eng-32` 并发化，则改为“提交协调器 + 等待队列”，把 `pending_commit_li` 交给单一批处理协程；不要在 `eng-33` 的每表队列里再做一层聚合队列。
- 分支/归档文件（`manifest_file`）与 `first_manifest`、`dict_dirty`、`manifest_limit` 走原有单条路径，不合并，避免语义分叉。

阶段 3：配置与指标

- `write_buffer_size` / `write_buffer_ratio` 要么接线到 `io-03` 的写缓冲池，要么删除/标注未实现；不要复用它们表达 group commit 参数。
- 新增 group 参数（建议与 C++ 无关，独立命名）：`write_group_max_puts`、`write_group_max_bytes`、`write_group_delay_us`。
- `fkv_metrics` 增补：`put_num`、`commit_num`、`avg_group_size`；`fsync_num`、`manifest_writes` 已存在（`fkv_metrics/src/lib.rs:324-326,283-286`）。

验收

- 单 key 单请求 1000 次：`fsync_num` 从约 2000 降到 `O(1000/group)`，且崩溃点注入后已确认 put 全部可恢复。
- 单次批量 1000 entry：不退化（仍约 2-3 次 fsync）。
- 组内任一请求失败：整组返回错误，`root_map` 与 manifest 无半提交。
- 读请求不被无限期饿死（barrier + `write_group_delay_us` 上限）。

### 风险

- **一致性**：ack 前必须完成 data fsync 与 manifest sync；否则丢失已确认写。合并 manifest 记录时漏掉 `dirty_page_li`/segment delta 会导致重启后页不可达。
- **回滚**：现在每 put 独立 `abort_batch`；分组后失败必须整组回滚，`OutOfMem` 重试逻辑（`fkv_shard/src/lib.rs:178-200`）需按组重放。
- **延迟**：等待成组会抬高 p99，必须有 `write_group_delay_us` 硬上限与 barrier。
- **可见性**：组提交完成前不得处理后续读；并发化后读快照与 COW 根切换需额外顺序保证（见 `27` / `eng-41`）。
- **跨表不可合并**：manifest 是每表一个文件，组只在同 table 内有效；多表混合负载收益递减。
- **与 `eng-33` / `eng-32` 的顺序耦合**：先做 `eng-33` 再改提交层，否则两套队列互相打架。
- **收益上限**：C++ 基线本身不做 group commit，因此这不解决任何兼容差异；若目标是基线对齐，应优先 `eng-49`、`io-03`、`io-06`。

### 明确不采纳的部分

1. “按 table 增加 write aggregator 队列”不复述、不实现：归 `eng-33`。
2. “C++ 并发写可合并”不作为事实引用：C++ 每分区单写任务，aggregator 只在请求内合并连续页。
3. “C++ manifest 与 data flush 有独立批处理节奏”删除：`write_task.cpp:855-868` 每任务固定顺序。
4. “每次 put 写 TTL page”改为“有 TTL 变更时写 TTL 树”。
5. `write_buffer_size` / `write_buffer_ratio` 不解释为 group commit 开关：它们在 C++ 是写缓冲池容量。

### 最终路径

`issue/todo/20-write-batching.md`（部分采纳，范围收敛为 group commit + 提交层指标）。

## 复核结论（2026-11-04，前置核验后）

### 判定

维持「部分采纳」，**本轮不落代码**，文件移入 `issue/todo/`。

理由：页 IO 与批内提交已被前置全部覆盖，唯一剩余增量是跨请求 group commit，
它必须改 `fkv_shard` 派发层，而该层本轮禁改且 `eng-33a` 仍阻塞。无可安全落地的子集。

### 一、前置核验（均已 done）

| 前置 | 现状 | 证据 |
|---|---|---|
| io-27 注册写槽位 + 在飞上限 | 已落 | `fkv_io_compio/src/lib.rs:60` `MAX_INFLIGHT_WRITE=16`、`:791` `WriteGate::new`、`FixedWritePool` |
| io-36 写组页数上限 | 已落 | `:62` `DEFAULT_MAX_WRITE_BATCH_PAGES=32`，`write_page_li` 按 `max_pages` 切分 |
| perf-82 跨组聚合 | 已落 | `fkv_task_write/src/lib.rs:69` `PENDING_WRITE_BUDGET = 1 MiB`，`PendingWrite` 跨组延迟写、一次 `write_page_li` |
| eng-33 per-partition WriteGate | 已落 | `fkv_shard/src/write_gate.rs` FIFO + `running` 门 |

页 IO 侧诉求（原文建议 1 的前半、影响 1 的 syscall 数）已无剩余。

### 二、提交层复核

1. **单次请求内已合并提交**。`put_at`（`fkv_store/src/lib.rs:1113`）→ `put_at_inner`（`:1139`）
   → 一次 `io.sync_data`（`:1267`）→ 一条 `append_mapping_log`（`:1295`）
   → `append_file_at` 内一次 `file.sync_data`（`fkv_io_compio/src/lib.rs:1569`）。
   100 entry 的批量 `put` 已是 O(1) 提交，原文建议 1 中「合并为一次 `write_batch` + 一次 append / 一次 fsync」
   在单请求粒度已成立。
2. **每次 put 2 次 fsync 成立**，且已计入 metrics：`MeteredIo::sync_data` 与
   `MeteredIo::append_manifest` 各调 `add_fsync`（`fkv_metrics/src/lib.rs:499,517`）；
   `manifest_writes` 由 `add_manifest_write` 计数。原文建议 4 的 fsync 次数观测已有口径。
3. **跨请求合并不可能**。`drain_write_gate`（`fkv_shard/src/lib.rs:1171-1197`）逐条
   `await run_gated_write`，`take_next` 后无并发在飞，攒批没有第二个请求可合并。
   `eng-33a` 已登记阻塞：`Store::put` 仍需 `&mut self`，依赖 `eng-32b`（未落地）。
4. **全仓无 `write_group` / `group_commit` 符号**，原文现状描述仍准确。

### 三、本轮不落代码的依据

- 页层（io-27 / io-36）、批内跨组（perf-82a）、每表写队列（eng-33）都已覆盖，页侧无增量。
- 提交层唯一增量是跨请求 group commit，落点只能是 `fkv_shard` 派发循环。
  本轮该目录有并行会话在改，且 `eng-33a` 前置不全，改了无法验收。
- store 侧先拆 `prepare_write` / `commit_write` 而不接消费方，等于制造死抽象，
  违反仓库「零死代码」约束。
- `put` 返回前不 fsync（延迟提交）会破坏「已确认的 put 崩溃不丢」，
  与本 issue 自身验收第 3 条冲突，不可作为子集。
- 因此安全子集为空。强行落任何一项都是「行为不变的重构」或「语义退化」，不做。

### 四、保留的验收口径（待 `eng-33a` / `eng-32b` 落地后重开）

- 单 key 单请求 1000 次：`fsync_num` 从约 2000 降到 `O(1000 / group)`，注入崩溃点后已确认 put 全可恢复。
- 单次批量 1000 entry：不退化，仍约 2 到 3 次 fsync。
- 组内任一请求失败：整组回滚，`root_map` 与 manifest 无半提交。
- 读请求不被组提交饿死：barrier + `write_group_delay_us` 硬上限。
- 调用方指引（现在即可用）：多 key 合并进一次 `put(tbl, batch)`，勿拆成 N 次单 key 请求。

### 五、去向

`issue/todo/20-write-batching.md`。重开条件：`eng-33a`（写并发派发）
或 `eng-32b`（per-table 状态拆分）落地，且 `fkv_shard` 不再有并行会话占用。

---

## DONE（2026-11-05）

- shard 在高优先通道收到写请求后先连续收集最多 64 条写请求，再统一进入
  `WriteGate`；`drain_write_gate` 对同一分区、同一 term 的连续 `Put` 调用
  `take_pending_while` 合并成一次 `Store::put`，共享一次 manifest append + fsync。
- 合并语义：
  - 单请求内部仍按 `Store::put` 的 entry_map 后写覆盖；
  - 跨请求同 key 取 timestamp 较大者，timestamp 相等后请求胜；
  - 不同 term / 非 Put 请求立即停止合并，保持 FIFO 与控制请求顺序。
- 新增：
  - `fkv_shard` 单测 `merge_put_batches_keeps_cross_request_timestamp_semantics`、
    `merge_put_batches_equal_timestamp_later_request_wins`；
  - `write_gate` 单测 `take_pending_while_preserves_partition_fifo`；
  - `fkv_runtime/tests/write_group_commit.rs`：256 个并发单 key put 的 fsync 次数
    < 128，且 scan 数量正确。
- 验证：workspace 584 tests passed, 1 skipped；fmt/clippy 通过。
- 遗留：`write_buffer_size` / `write_buffer_ratio` 仍不是运行期配置；
  跨请求 group commit 目前只合并高优先、同 term 的 `Put`。

## 复审（2026-11-05 r6）

结论：**通过**。group commit 已落地且验收断言覆盖。

- `fkv_shard/src/lib.rs:62-65` `MAX_PUT_GROUP_REQUESTS` 常量；`:1569-1607` `drain_write_gate` 对同分区、同 term、高优先连续 Put 调 `take_pending_while`（:1575）合并；不同 term / 非 Put 停止合并（:1576 谓词）。
- 合并语义 `merge_put_batches`（:1623-1643）：请求内后写覆盖（local BTreeMap :1629-1632），跨请求 `entry.timestamp >= old.timestamp` 才覆盖（:1634-1636，相等后请求胜）。
- 单测：`merge_put_batches_keeps_cross_request_timestamp_semantics`（:2460-2491，含旧 ts 不覆盖、单请求内后写覆盖）、`merge_put_batches_equal_timestamp_later_request_wins`（:2494-2502）；write_gate 单测 `take_pending_while_preserves_partition_fifo`（write_gate.rs:256-273）。
- 集成测试 `fkv_runtime/tests/write_group_commit.rs:50-85`：256 并发单 key put，断言 fsync 总数 < 128（:74-77）且 scan 数量 = 256（:78-81）。
- 遗留（write_buffer_size/ratio 仍非运行期配置）与 md 一致。

## 更正（2026-11-05）

- 上文 DONE「单请求内部仍按 `Store::put` 的 entry_map 后写覆盖」（:217）与 r6 复审
  「合并语义 … 请求内后写覆盖（local BTreeMap）」（:235）与在盘实现不符：group commit
  合并（现名 `merge_put_batch_li`）**请求内同 key 亦按 timestamp 折叠**——ts 大者胜、
  相等时批内后写胜，与跨请求同为 `timestamp >=` 口径。
- 规范依据：单条 Put 路径 `Store::put` → `prepare_write_batch`
  （`fkv_task_write/src/ttl.rs:89`）→ `write_batch`（`fkv_task_write/src/lib.rs:224`）
  同批重复 key 即按 `timestamp >=` 取较新者；C++ `src/tasks/batch_write_task.cpp:569`
  `change_ts >= base_ts` 亦以 ts 定胜负。旧 `local` BTreeMap 的无条件覆盖会与单请求
  dispatch 路径分叉。
- 原「entry_map 后写覆盖」是 group commit 落地时的旧口径记录；实现已于同日按 ts 折叠
  收敛，测试断言相应改为请求内 ts=5 胜 ts=4（value=first）。详见
  `issue/reviewed/eng-90-merge-put-batches-per-entry-clone.md`「复审（2026-11-05 v2）」。
