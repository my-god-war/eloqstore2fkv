# 待办总表（issue/todo）

- 更新日期：2026-11-05
- 来源：本轮 `issue/new/` 的 21 条意见 + 账本剩余任务 + L1 升级前置
- 流转规则见 `issue/run.md`：甄别 → 开发 → 跑测试 → 移 `issue/done/`；不成立移 `issue/reject/`

## A. 本轮 review 已接收入开发队列（2026-11-05 已完成 10 / 拒绝 1）

| 编号 | 任务 | 优先级 | 结果 |
|---|---|---|---|
| perf-01 | scan 复用整页缓冲池 | P1 | **done**：[记录](../done/perf-01-scan-page-buffer-reuse.md)（整页分配 2→0） |
| perf-02 | 索引页并入批内写缓冲 | P1 | **done**：[记录](../done/perf-02-index-page-write-batch.md)（8 次 → 1 次） |
| perf-05 | 写合并零拷贝 | P1 | **done**：[记录](../done/perf-05-put-merge-move.md)（按幸存者分配 8192→0） |
| test-01 | 页写失败后可用性测试 | P1 | **done**：[记录](../done/test-01-page-write-failure-recovery.md)（SimStore 部分写 + Compio 回执） |
| dead-01 | 清理 3 个死 API | P2 | **done**：[记录](../done/dead-01-unused-pub-api.md)（删 4 个死函数） |
| test-04 | 删废话测试 | P2 | **done**：[记录](../done/test-04-useless-tests.md)（删 2 用例 + 1 常量自证行） |
| sem-01 | 单批重复 key 语义文档化 | P2 | **done**：[记录](../done/sem-01-batch-duplicate-key.md)（COMPAT_BASELINE + 4 测试） |
| io-02 | fdatasync 并发上限接配置 | P2 | **done**：[记录](../done/io-02-fsync-inflight-cap.md)（64 脏文件峰值 16→64） |
| io-03 | S3 worker panic 不被吞 | P2 | **done**：[记录](../done/io-03-s3-worker-panic-swallowed.md)（panic → IoFail） |
| test-02 | scan 损坏页检测 | P2 | **done**：[记录](../done/test-02-scan-corrupted-page.md)（scan + keys_only + get） |

> io-01 已 **reject**：shard sender 的 deadline / 背压定时器跑在调用方执行器
> （pollster / tokio 等），改 compio 定时器会 panic 且使请求 future 变 `!Send`；
> 证据见 [reject](../reject/io-01-deadline-timer-futures-timer.md)。

## B. 2026-11-05 复核批次（5 条，全部闭环）

| 编号 | 任务 | 优先级 | 结果 |
|---|---|---|---|
| perf-06 | 映射代际 per-Store | P2 | **done**：[记录](../done/perf-06-mapping-generation-per-shard.md)（`GENERATION_SOURCE` 归零，`GenerationSource` 由 Store 持有；核对修正——代际取点 6 处含回滚锚点 `:781`，验收改为「不触碰全局原子」断言） |
| struct-02 | IO 转发宏 | P2 | **done**：[记录](../done/struct-02-io-forwarding-macro.md)（三装饰器宏化 -614 行；`Self::Local(io)` 56→4、`store_of(tbl)` 44→6、`self.inner.` 52→26，实测吻合） |
| struct-03 | 拆巨型文件 | P2 | **done**：[记录](../done/struct-03-split-giant-files.md)（五个文件全部拆完：fkv_io_compio 3433→448、fkv_store 4358→328、fkv_shard 4018→535、fkv_runtime 4556→255、fkv_cloud 5864→125） |
| struct-04 | 拆巨型函数 | P2 | **done**：[记录](../done/struct-04-split-giant-functions.md)（dispatch 417→42、start 433→69、start_impl 339→35、clean_expired 212→58，356 测试绿） |
| test-03 | sim 故障矩阵 | P2 | **done**：[记录](../done/test-03-sim-fault-matrix.md)（`fault_matrix.rs` 3 测试、≥5 op；`clear_fault_li` 对齐 C++ `FailPoint::Disarm`） |

> 本轮 21 条意见全部闭环：**20 done / 1 reject**。
> 核对中发现的偏差（io-01 的 reject 理由、perf-06 的 6 个取点与验收修正、
> struct/test 类行号漂移）已并入对应 done 记录。
> 工作区状态：fkv 仓在收尾时被外部进程重建为单条 `init` 提交（HEAD `6d962f0`），
> 全部源码改动已包含在该提交中，`git status` 干净；eloqstore2fkv 仓的 issue/文档/语料
> 变更仍未提交。**建议按 issue 补拆分提交或保留该快照**。

## C. 账本剩余（`.data/motedb.mote`，`fkv_ledger stats`）

真实剩余 **312** 个 unit（另有 231 个范围外 ignored，0 个 verified）：

| crate | todo | crate | todo |
|---|---:|---|---:|
| fkv_io | 92 | fkv_page | 22 |
| fkv_shard | 58 | fkv_codec | 14 |
| fkv_buf | 30 | fkv_runtime | 13 |
| fkv_types | 24 | fkv_task_write | 13 |
| fkv_mapping | 21 | fkv_btree | 10 |
| fkv_failpoint | 8 | fkv_config | 3 |
| fkv_object | 2 | fkv_standby | 2 |

「下一批任务」见 `doc/progress.md` 末尾 12 条（`include/async_io_manager.h` 的 AsyncIoManager、
`include/circular_queue.h`、`include/pool.h`、`include/tasks/write_buffer_aggregator.h` 等）。

## D. L1 升级前置（`COMPAT_BASELINE` 证据段）

1. **sto-51** 撕裂尾覆盖语义修复；
2. **V2/V4** 引擎级双向互读系统化执行（含续写）；
3. 完成后把 `COMPAT_BASELINE` 的 `L1-partial` 升为「已验证」，并跑
   `cargo run -p fkv_ledger -- report --out doc/progress.md` 刷新看板。

当前账本 `verified` 为 **0**（1305 done），`verified + ignored` 覆盖率 12.5%，
L1 仍是 `partial`。

## E. 待澄清（`questions.md`）

- **Q1**：312 个真实剩余先 `scaffold` 生成骨架，还是先补 `verify` 逐 unit 证据闭环；
- **Q5**：引擎级 V3/V4 用逐字节还是结构等价，哪些 `_cpp` crate 进常规 PR 门禁；
- **Q7**：`perf-baseline.json` 是否合并回 web 看板的数据层。

## F. 建议执行顺序（2026-11-05 复核更新，A/B 批次已全部闭环）

1. fkv 仓已被压成单条 `init` 提交（`6d962f0`，工作树干净）：按 `done/` 记录把
   本轮改动补拆成 per-issue 提交（拆分类与行为类分开），或明确保留该快照并
   在 doc 记录取舍；
2. 账本 `todo` 的 312 个 unit 按 `fkv_ledger next` 顺序推进，优先 `fkv_io`（92）与
   `fkv_shard`（58）；动工前澄清 E 节 Q1；
3. L1 升级前置（D 节）：sto-51 修复 + V2/V4 引擎级互读系统化执行，完成后刷新看板；
4. eloqstore2fkv 仓自身的 issue/文档/语料变更尚未提交，择机落库；
5. `unused.js` 每轮仍报 2 条已核实的 trait impl 误报（dead-01 风险节）：如需清零
   走工具侧通用 `--ignore`，不要在共享脚本里写死 fkv 路径。

## G. 2026-11-06 收口状态

- 账本（`.data/motedb.mote`）：**todo 0** / done 1461 / ignored 387 / verified 0。
  所有 312 个真实剩余 unit 已逐簇完成 done/ignored 裁定（fkv_btree / fkv_io / fkv_shard /
  fkv_buf / fkv_types / fkv_page / fkv_mapping / fkv_codec / fkv_runtime / fkv_task_write /
  fkv_failpoint / fkv_config / fkv_object / fkv_standby）。
- 两轮 review 意见：21 条首轮 + 18 条复审 + 账本核对中新发现的 rt-01/rt-02/fp-01/
  fkv_config-01/sem-08，除下列 4 条 P3 外全部 done/reject。
- 全量门禁：`cargo nextest run --workspace --all-features` = 1117 passed / 1 skipped；
  `cargo nextest run -p fkv_compat` = 12 passed。
- 剩余 `issue/new`：
  - `struct-14-temproot-inline-stragglers.md`（P3，7 个测试文件内联临时目录迁移）
  - `perf-11-bench-lanes-ttl-compact.md`（P3，bench 增加 TTL/compact lane）
  - `struct-16-bench-main-2174.md`（P3，fkv_bench/main.rs 拆分）
  - `sem-08-compression-disabled-after-reopen.md`（P2，test-10 发现的写入门控差异）
- 仍未完成：L1 升级前置（sto-51 撕裂尾 + V2/V4 引擎级双向互读）、`verified` 仍为 0。

## H. 2026-11-07 最终收口（核对补充，G 节为昨日快照）

- **L1 已升级为「已验证」**：`COMPAT_BASELINE` 顶部现为
  「L1（Local + append/pooled 分配 + 无压缩；引擎级 V4 双向与 V3 oplog 差分已执行，
  见 doc/engine-cross-evidence.md）」——G 节的「L1 未升级 / verified 0」已过时。
- 账本：todo 0 / done 1435 / ignored 387 / **verified 26**。
- 本轮（11-06~07）并行流程已闭环：`issue/reviewed/` 177 条 + `issue/done/` 21 条 +
  `issue/reject/` 1 条（io-01）。`issue/new/` 仅剩 1 条 P3。
- 门禁实测：`cargo nextest run --workspace --all-features` = **1462 passed / 1 skipped**；
  `cargo nextest run -p fkv_compat` = **12 passed**（双向）。
- `issue/new` 剩余：`sem-52-delete-middle-page-predecessor-record-order.md`（P3，
  data mapping log 记录顺序残余差异，证据链已核实：sem-51 done 记录 +
  `deferred_prev` 机制确认，仅 manifest 内 UpdateMapping/DeleteMapping 先后与
  C++ `ShiftLeafLink` 不同，终态一致）。其引用路径写 `issue/reviewed/sem-51` 而实际
  在 `issue/done/sem-51`，属笔误，证据实质成立。
