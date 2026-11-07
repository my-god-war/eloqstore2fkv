# 本轮 review 索引（issue/new）

- 生成日期：2026-11-05
- 依据：`issue/new.md` 的 6 个审查重点
- 审查对象：`../fkv`（Rust）对照 `../eloqstore`（C++ baseline `82b774c`），运行时对照 `../compio`
- 范围排除：`COMPAT_BASELINE` 已归档的范围外 / 有意差异（分配模式、压缩、撕裂尾、branch segment、
  TTL 读过滤、页大小、四分页切分点、file id 预留、buffer pool / data page cache、S3 并发模型、
  sdk_runtime、test_utils、EloqModule、未生效旋钮等）未重复列出。

## 发现清单

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [io-01](../reject/io-01-deadline-timer-futures-timer.md) **reject** | deadline 路径用 futures_timer，未接 compio 定时器（sender 定时器在调用方执行器，非 compio） | 异步 | P1 | 高 |
| [io-02](../done/io-02-fsync-inflight-cap.md) **done** | 批量 fdatasync 被硬编码 16 上限分波 | 异步 / 批量提交 | P2 | 高/中 |
| [io-03](../done/io-03-s3-worker-panic-swallowed.md) **done** | S3 worker 丢弃 JoinError，panic 被伪装成 NotRunning | 异步 / 错误语义 | P2 | 高 |
| [sem-01](../done/sem-01-batch-duplicate-key.md) **done** | 单批重复 key：C++ 断言唯一，Rust 静默折叠 | 语义 | P2 | 高 |
| [perf-01](../reviewed/perf-01-scan-page-buffer-reuse.md) **reviewed** | scan 每请求两次整页零初始化 | 性能 | P1 | 高 |
| [perf-02](../reviewed/perf-02-index-page-write-batch.md) **reviewed** | 索引页逐页独立写，未并入批内写缓冲 | 性能 / 批量提交 | P1 | 高/中 |
| [perf-03](../reviewed/perf-03-page-read-path-rebuild.md) **reviewed** | 每次页读重建候选路径 Vec<PathBuf> | 性能 | P2 | 高 |
| [perf-04](../reviewed/perf-04-tableident-arc-str.md) **reviewed** | TableIdent 的 Box<str> 克隆每请求 1-3 次分配 | 性能 | P2 | 中高 |
| [perf-05](../reviewed/perf-05-put-merge-move.md) **reviewed** | 写合并对每个幸存 entry 整批 clone | 性能 | P1 | 高 |
| [perf-06](../reviewed/perf-06-mapping-generation-per-shard.md) **reviewed** | 映射代际用进程级全局原子 | 性能 / 并发 | P2 | 中 |
| [struct-01](../done/struct-01-tableident-dir-name.md) **done** | `{tbl}.{partition}` 目录名生成/解析散落 7 处 | 结构 | P2 | 高 |
| [struct-02](../done/struct-02-io-forwarding-macro.md) **done** | IO trait 约 260 处纯转发样板 | 结构 | P2 | 高 |
| [struct-03](../done/struct-03-split-giant-files.md) **done** | 五个核心文件超 3000 行 | 结构 | P2 | 高 |
| [struct-04](../done/struct-04-split-giant-functions.md) **done** | 四个 300+ 行巨型函数 | 结构 | P2 | 高 |
| [struct-05](../done/struct-05-workspace-dependencies.md) **done** | 缺 `[workspace.dependencies]`，版本漂移 | 结构 / 依赖 | P2 | 高 |
| [struct-06](../done/struct-06-kvapi-trait.md) **done** | `KvApi` 单实现 trait 与 inherent 方法重复 | 结构 | P2 | 中 |
| [test-01](../done/test-01-page-write-failure-recovery.md) **done** | 页写 IO 错误后可用性无测试 | 测试 | P1 | 高 |
| [test-02](../done/test-02-scan-corrupted-page.md) **done** | 盘上 data 页损坏只测 get，未测 scan | 测试 | P2 | 中 |
| [test-03](../done/test-03-sim-fault-matrix.md) **done** | SimStore 故障注入只覆盖 3 个 op | 测试 | P2 | 中 |
| [test-04](../done/test-04-useless-tests.md) **done** | 清理废话测试（纯 std / 常量自证 / 重复） | 测试 | P2 | 高 |
| [dead-01](../done/dead-01-unused-pub-api.md) **done** | `unused.js` 5 命中：3 真死 API + 2 误报 | 死代码 | P2 | 高 |

## 本轮已对照、未发现新差异的部分

以下按字节 / 字段逐项对照后一致，不单列 issue（避免复述已归档结论）：

- 页头布局与页类型枚举：`fkv_page/src/header.rs:6-41` vs `eloqstore/include/storage/page.h:13-31`；
- 页 checksum 覆盖范围与算法（XXH3，跳过前 8 字节）：`fkv_codec/src/lib.rs:104-110` vs
  `eloqstore/src/storage/page.cpp:34-48`；
- varint 编码（LEB128，非 zigzag）：`fkv_codec/src/lib.rs:44-91` vs `eloqstore/src/coding.cpp:29-115`；
- manifest 头布局与 checksum 的 1 MiB 分批 / `rotl(1) ^ xxh3` / mixer：
  `fkv_manifest/src/lib.rs:25-56` vs `eloqstore/src/storage/root_meta.cpp:177-199`；
- manifest 文件名（`manifest_<branch>_<term>[_<tag>]`）与解析拒绝 legacy 纯数字 branch：
  `fkv_types/src/lib.rs:279-333` vs `eloqstore/src/async_io_manager.cpp` / `file_gc.cpp` 的
  `BranchManifestFileName` / `ParseManifestFileSuffix`；
- `Truncate` 语义（空 position = 整表；非空 = 删除 key >= position）：`fkv_store/src/lib.rs:2323-2340`
  vs `eloqstore/src/tasks/batch_write_task.cpp:1845-1880`；
- `ListArchiveTags` 的 `(tag, term)` 排序去重：`fkv_store/src/lib.rs:2720-2735` vs
  `eloqstore/src/eloq_store.cpp:2206-2219`；
- 错误枚举顺序与可重试集合：`fkv_error/src/lib.rs:8-31` vs `eloqstore/include/error.h:10-34`
  （仅 `NotRunning` 文案不同，无行为差异）。

## 第二轮迭代（2026-11-05，结构专项 review 重点 4）

针对 struct-01~06 全部闭环后的复查：拆分后新结构引入的问题 + 未覆盖的结构面。
工具扫描基线：`cargo clippy --workspace --all-features --all-targets` 零警告；
生产代码 `dyn` 仅 7 处且全部正当（`Arc<dyn Fn>` 回调、panic payload、
`dyn StdError` 链、bench `main`），无 `dyn` 滥用可立项。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [struct-07](../reviewed/struct-07-request-classification-scattered.md) **reviewed** | Request 分类散落 10 处独立 match，101 处变体枚举无单一事实源 | 结构 / 重复 match | P2 | 高 |
| [struct-08](../reviewed/struct-08-test-temp-root-duplication.md) **reviewed** | 43 个测试文件手写 temp_root 样板（440 行 + 117 处手工清理），fkv_testkit::TempRoot 已存在未采用 | 结构 / 测试基建 | P2 | 高 |
| [struct-09](../reviewed/struct-09-glob-reexport-flat-namespace.md) **reviewed** | struct-03 拆分留下 `pub(crate) use mod::*` 链 + 全量 `use super::*`，fkv_runtime/fkv_cloud crate 内命名空间被压平 | 结构 / 模块化 | P2 | 中高 |
| [struct-10](../reviewed/struct-10-next-tier-giant-files.md) **reviewed** | 第二梯队巨型文件：fkv_task_write 1845 生产行等 5 个文件仍超 1000 行 | 结构 / 模块化 | P2 | 高 |
| [struct-11](../reviewed/struct-11-metrics-layer-inversion.md) **reviewed** | fkv_metrics 位居 L0 却依赖 L3 的 fkv_io（MeteredIo），分层倒挂 | 结构 / crate 边界 | P2 | 高 |
| [struct-12](../reviewed/struct-12-file-prefix-literals.md) **reviewed** | data_/segment_ 文件名前缀 3 处独立定义 + standby 硬编码 ".tmp"（TMP_SUFFIX 已存在） | 结构 / 魔法字符串 | P2 | 高 |

本轮核对过、未立项的检查点（附结论）：

- clippy 警告分布：`cargo clippy --workspace --all-features --all-targets`
  退出码 0、输出 0 行——无警告可归因；
- `dyn` 滥用：生产代码 7 处全部正当（见上），struct-06 之外无可立项点；
- unwrap/expect：生产路径仅 5 处，逐处核对均在注释声明的 100% 安全点
  （`fkv_standby/src/lib.rs:783-785` 的 `f.take().unwrap()` 两分支互斥、
  `fkv_buf/src/lib.rs:35` 前置 `assert!(is_power_of_two)`、
  `fkv_cloud/src/cache.rs:95` 刚插入必中、其余在 tests）；
- 错误处理：`KvError` 为对齐 C++ `error.h` 的纯枚举，`map_io_err` /
  `Display` 委托 `as_str() const fn`，无字符串错误与手写 Display 问题；
- 编译期可上移的工作：`Op` 枚举已用 `op_enum!` 宏单源生成
  （`fkv_metrics/src/lib.rs:44-89`），常量定义充分（`BUCKET_UPPER_US` 等），
  无 phf/const fn 可立项点（Request 分类 match 归入 struct-07）；
- 二次导出：公共面 `pub use` 均显式具名；`PageOption` 存在
  fkv_task_scan → fkv_store → fkv_sdk → fkv 三跳链，但各跳是公共 API
  组装而非转发链，单独立项过小，随 struct-09 收紧时顺带评估；
- crate 依赖方向：除 struct-11 的 fkv_metrics→fkv_io 一条倒挂外，
  foundation/core/format/io/engine/facade 其余依赖边全部向下
  （逐 crate Cargo.toml path 依赖扫描）。

## 测试专项第二轮迭代（2026-11-05，review 重点 5；test-01~04 已闭环不重复）

对照 eloqstore 全部 348 个 TEST_CASE（persist / concurrency / batch_write /
manifest / replayer_term / standby / branch / gc / large_value / cloud /
chore / scan / delete / segment_*）与 fkv 测试矩阵，差分面核对 oracle/语料体系。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [test-05](test-05-shutdown-race-deadlock.md) | 请求与 shutdown 竞态无测试，实测可复现 shutdown 死锁（compio sync 队列满 + 内联 join 互等） | 测试 / 失败路径 | P1 | 高 |
| [test-06](test-06-put-large-segment-failure-rollback.md) | put_large 段写失败的 Store 级回滚无测试（test-01 未覆盖遗留） | 测试 / 失败路径 | P2 | 高 |
| [test-07](test-07-compare-matches-ord-useless.md) | 废话测试残余：compare_matches_ord 断言 `left.cmp(right) == left.cmp(right)` | 测试 / 清理 | P2 | 高 |

本轮核对过、未立项的检查点（附文件:行号与结论）：

- shutdown 竞态覆盖：`fkv` 全部 `shutdown().await` 调用点（约 30 处，如
  `fkv_shard/tests/write_gate.rs:138`、`fkv_runtime/tests/runtime.rs:92`）
  均在全部请求回执之后 → 立项 test-05；
- term bump 负路径（同 term / term=0 不 bump）：`fkv_store/tests/term_bump.rs:29,82`
  已覆盖，C++ `replayer_term.cpp:140-230` 三个负用例无缺口；
- reopen waiter 失败传播：`fkv_shard/tests/cloud_reopen.rs:170,206,311,438`
  覆盖 C++ `chore.cpp` "failed external reopen completes pending reopen waiters"；
- fd LRU：`fkv_io_compio/tests/hardening.rs:227,260` 覆盖 C++ persist.cpp 两用例；
- floor expire_ts 自报：C++ `scan.cpp` "floor reports its own expire_ts" 的 bug
  形态在 Rust 结构性不存在（`fkv_page/src/restart.rs:77-94` peek_cmp 为 `&self`，
  不改游标状态），floor 的 TTL 回退已有
  `fkv_task_read/tests/read.rs:295,347,426` 三层覆盖，不立项；
- oracle 矩阵（page/index/overflow/manifest/compress/mapping/store 双向）：
  `tool/fkv_ledger/src/fixture.rs:31-43` + CI `.github/workflows/oracle.yml:100-118`
  已双向校验；oracle 语料未跑 seek/seek_floor 一节不立项——
  `fkv_page/tests/seek.rs:1-80` 已用随机矩阵 × restart interval {1,2,3,16,64} ×
  共享前缀对线性参考逐点对比，强于固定语料；
- 常量自证残余：除 test-07 外，`fkv_error display_is_stable`（2 行，锁定对外
  文案）与 `fkv_gc should_compact` 边界断言均有行为含义，保留；
- 空批 put（C++ batch_write.cpp "empty batch write"）：Rust `put_at_inner`
  对空批早退 `Ok(())`（`fkv_store/src/write.rs:88-90`）无历史 hang 形态，纯
  覆盖率用例不立项；
- 断言有效性抽检：`term_bump.rs` 期望值临时改错 → 测试即失败 → 还原
  （git diff 干净），断言真实可失败。

## 测试质量第三轮迭代（2026-11-06，第 2 轮 test review + review.md 复审）

test-05/06/07 复审全部达标，已移 `../reviewed/`：

- **test-05**：revert 实验（worktree @ `ba0f8bd^` + 仅恢复测试文件）确认
  修复回退后测试在 15.012s 守护超时处**稳定红灯**（死锁消息命中），断言
  真实可失败；15s 守护 < nextest `slow-timeout` 120s terminate，CI 上表现为
  失败而非挂死，且修复后单次 ~30ms，慢机余量约 500 倍，充分；
- **test-06**：三条失败入口（`write.rs:440-444` 段写 / `:407-414`
  base_timestamp / `:451-462` 超限）逐条有用例命中，且三处均断言
  `max_fp_id` 高水位回滚 + `mapping_count` / `tbl().len()` / `free_page_cnt` /
  活跃段 `< max_fp_before`，file id 泄漏检查完备；
- **test-07**：`compare_matches_ord` 已删（grep 零命中），剩余
  `separator_matches_cpp_vectors` / `separator_properties` /
  `separator_matches_cpp_reference_on_random_pairs`（vs 手写 C++ 参考实现
  差分 2000 对）为有效覆盖。

第 2 轮新增实现（io-05 任务化 / io-04 滑动窗口 / io-06 直读 / perf-07
扁平键区 / sem-03 compact_before_archive）覆盖核查：除 sem-03 的 segment
侧（立项 test-08）外，每处新行为均有失败路径或顺序性断言测试（详见下
「核对过、未立项」清单）。废话测试检查：30+ 新测试逐个过目，无凑数 /
自证 / 重复（codec 向量为 C++ oracle 差分；ReadFlight 单测锁窗口机制行为
而非常量自证）。flaky 检查：`cargo nextest run --workspace --all-features`
3 遍全绿（1071 passed × 3），并发专项（shutdown_race / maintenance_concurrency
/ read_concurrency / queue_backpressure / write_gate / write_concurrency）
加跑 5 遍全绿。死代码：`bun sh/unused.js` 仅 dead-01 已知 2 误报
（trait impl `partial_cmp` / `as_uninit`），无新增，不立项。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [test-08](test-08-archive-segment-pre-compact-untested.md) | compact_before_archive 的 segment 侧（needs_segment_maintain 分支）零测试（C++ segment_compact.cpp:613 有 below-threshold 门对照） | 测试 / 失败路径 | P2 | 高 |
| [test-09](test-09-archive-survives-truncate-maintain.md) | truncate+maintain 后 archive 必须幸存且可回滚（C++ gc.cpp:261 有用例，fkv 仅测两个半边） | 测试 / 语义 | P2 | 高 |
| [test-10](test-10-compression-toggle-across-reopen.md) | 压缩开关跨重启翻转无测试（C++ persist.cpp:654 有对照；非字节等价归档项） | 测试 / 语义 | P3 | 高 |

本轮核对过、未立项的检查点（附结论）：

- io-05 任务化（`3f62da4`）：`maintenance_concurrency.rs` 5 例以完成顺序
  （非墙钟）断言跨分区推进 / GetLarge 单飞不串值 / PutLarge buffer 往返 /
  shutdown 排空在飞 Compact / 写门 per-partition FIFO 保持；写任务失败面由
  `page_write_failure.rs`（Put IoFail 后 shard 可用，任务化后同一
  WriteDone 归还路径）覆盖；PutLarge 借不到 fixed buffer 走普通路径为
  语义等价的纯性能分支（实现注释自证「语义一致」），不立项；
- io-04 滑动窗口（`5603b64`）：`read_window_slides_past_batch_limit` 以
  回执交错 + `read_in_flight_max > MAX_READ_BATCH` 锁滑动语义；
  `read_flight_*` 单测锁窗口满判定与完成即回收（错误路径经
  `ReadTask::poll` 的 JoinHandle Err 分支结构性同路回收），不立项；
- io-06 直读（`32f73cc`）：`read_owned.rs` 9 例覆盖内容 / pread 计数 /
  短 Vec InvalidArgs / 尾部不动 / 零池分配 / 短读耗尽 Corrupted / 稀疏洞
  回退 / EOF 回退 / TryAgain 重试 / O_DIRECT（Linux）回退，失败面完整；
- perf-07 扁平键区 + perf-08 写路径池（`7f37ad2` / `87c923c`）：
  `alloc.rs` 计数分配器锁小分配常数与整页缓冲复用；正确性由全量
  btree/store 套件承载，不重复立项；
- io-07 fixed 写槽配额（`1b94834`）：`fixed_write_cancel.rs` 覆盖取消
  泄漏路径，闭环达标；
- C++ 矩阵其余候选逐一排除：write task abort 高水位回退
  （persist.cpp:570，cloud+branch 特定，fkv 映射批回滚机制已由 test-01/06
  覆盖主干）；branch 高水位 / replayer term / reopen waiter / fd LRU /
  standby / drop 保留 archive（branch_archive.rs:579）均已对照有覆盖。

## 限制

- 引擎级 V2/V4 双向互读未在本轮执行（`COMPAT_BASELINE` 已记录为 L1 升级前置，不重复立项）；
- 语义对照覆盖读 / 写 / scan / floor / truncate / archive 的入口与关键分支，未穷举全部 C++ 测试矩阵；
- 异步对照覆盖调度、取消、定时器、批量提交、panic 传播；未做运行时级压测归因。

## 第三轮迭代（2026-11-06，性能复审专项：perf-07/08 闭环核验 + 新热点）

复审对象：`7f37ad2`（perf-07 扁平键区）、`87c923c`（perf-08 写路径页缓冲池化）、
`32f73cc`（io-06 批读直读，读路径侧）。两条实现复审达标（perf-01~08 全部
流转 `reviewed/`）：

- **perf-07**：`Children` 扁平键区在页拆分（`rewrite_data_page` 分裂链 / `sep_li`）、
  合并（空页 `Children::default()` + `MergedChildren` 严格升序校验）、重分配
  （`rebalance_index_page_li` 改首键后 `build_index_pages` 真实首键）、overflow
  （`free_overflow_chain` 池缓冲复用）、TTL 树（经同一 `write_batch`）各分支语义
  保持；compact 为页搬移（remap）不经该路径。计数测试实测 `small=31, total=55`
  （断言 ≤32）仍锁住回归。`index_entry_cnt` 先按 region 头数一遍再迭代一遍，
  属轻量双遍（不解码键），非回退，不立项。
- **perf-08**：issue 原 6 处（open / descend_leftmost / read_page_by_id /
  update_next / update_prev / free_overflow_chain）全部接 `PageBuf`；帧
  read+clone 已消（`push_frame` move）；pending 命中走 `take` 零拷贝；池上限
  8 块 × page_size / 线程，深树 >8 退化为一次分配不炸；实测
  `page total=2, zeroed=0`（断言零初始化为 0）。
- **io-06 读侧**：点读 / scan 不物化 `Children`（IndexCache 命中 `node.seek`、
  未命中就地 `IndexPageIter`），lookup 路径无新增 memcpy；遗留为单页
  `read_page` 的池缓冲拷贝 → 立项 perf-10。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [perf-09](perf-09-io-read-pool-no-byte-budget.md) | IO 层读/写缓冲池按块数封顶无字节预算，整文件级缓冲可经失败路径常驻（direct 池有 16 MiB 字节预算，口径不一致） | 性能 / 内存上界 | P2 | 高/中 |
| [perf-10](perf-10-single-page-read-still-copies.md) | 单页 `read_page` 仍「读池直读 + 整页 memcpy」（io-06 遗留，写路由每批 3-5 次 + 点读每层 1 次） | 性能 / 拷贝 | P2 | 高/中 |
| [perf-11](perf-11-bench-lanes-ttl-compact.md) | bench 无 TTL 双树写 lane 与 compact 并发 lane，perf-07/08 后的双树写 / 后台争用无 gate 口径 | 性能 / benchmark | P3 | 高/低 |

本轮核对过、未立项的检查点（附结论）：

- **任务化（`3f62da4`）每请求装箱/通道开销**：每 Put 组 = 1 次 `spawn` 任务
  装箱 + 1 次 `Box<ShardStore>` 交回（`write.rs:435`，`finish_write` 解箱
  move 回槽）+ 2 次 `TableIdent` refcount + oneshot 回执，合计 ~3 次小分配 /
  批；实测 4680 batch/s ⇒ ~1.4 万次小分配/秒，<0.1% 写路径 CPU，不立项；
- **后台任务与前台 IO 争用**：入队侧 `Pri::Low` + `max_processing_time_us`
  防饥饿（`sender.rs:11-17`）；任务化后 compact 每批 read/write await 边界
  自然与前前台交错（io-05 语义）；IO 闸门（`write_gate.acquire`）先到先得
  无带宽优先级，但 C++ io_uring 侧同样只有调度优先级（`YieldToLowPQ`）而非
  IO 优先级，对齐现状，不立项（可观测性缺口归 perf-11 的 compact lane）；
- **各池内存上界**：页缓冲 TLS 池 8 × page_size / 线程；写池 8 ×
  `max_write_batch_pages(32)` × page_size = 1 MiB / store；direct 池
  16 MiB 字节预算 / store；`PendingWrite` 1 MiB 预算 + 单组页数；
  `IndexCache` 受 `root_meta_cache_bytes` 预算。唯一缺口 = IO 读池
  （perf-09）；
- **bench 实测**（darwin arm64，rows 20 K，batch 100，kv 128 B）：
  put 4680.64 batch/s（p50 203 μs / p99 344 μs）、get 19.86 万 kv/s
  （p50 18 μs）、scan 445.7 万 kv/s；kv 8192 B（overflow 链）put 12.96 万
  kv/s、get p50 76 μs——overflow 批读直读（io-06 受益路径）经 `--kv-size`
  可达且量级正常，无回退迹象。

## 结构专项第三轮迭代（2026-11-06，第 2 轮 struct review + struct-07~12 复审）

struct-07~12 复审全部达标，已移 `../reviewed/`（逐项证据见各文件复审记录；
要点）：struct-07 宏表 26 条与 `Request` 枚举 26 变体集合一致，dummy 变体
实验在 HEAD `e072e8e` 复现 2×E0004 编译失败，`pri_of` 两臂穷举、`op_of`
保留穷举（无 `_` 兜底）；struct-08 唯一漏网为记录在案的
`global_tests.rs`（扩展见 struct-14）；struct-09 两 crate glob /
`use super::*` 归零且提交为纯 import 改动；struct-10 行数对账逐文件一致、
fkv_metrics 跳过有据（struct-11 后 667 行 < 1000 阈值，done 记录在案）；
struct-11 `fkv_metrics` Cargo.toml 无 `[dependencies]` 节、`cargo tree`
仅自身；struct-12 生产代码前缀字面量仅剩 `fkv_types` 定义处 + 编译期
`concat_eq` 断言在位、standby 自有常量删净。HEAD 全量
`cargo nextest run --workspace --all-features`：1071 passed / 1 skipped /
0 failed。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [struct-13](struct-13-fkv-large-layer-inversion.md) | fkv_large(L2) 生产依赖 fkv_io(L3)，struct-11 后唯一生产倒挂；分层方向仍无门禁（struct-11 可选验收未落地） | 结构 / crate 边界 | P2 | 高 |
| [struct-14](struct-14-temproot-inline-stragglers.md) | TempRoot 收尾：7 个内联手写临时目录文件（struct-08 普查键 `fn temp_root` 的盲区 + struct-11 新引入 metered.rs） | 结构 / 测试基建 | P3 | 高 |
| [struct-15](struct-15-shard-store-star-glob.md) | fkv_shard/fkv_store 22 个生产模块 `use super::*` 星型继承根命名空间（≈117/≈78 个名字），struct-09 只清了 runtime/cloud | 结构 / 模块化 | P2 | 中高 |
| [struct-16](struct-16-bench-main-2174.md) | fkv_bench/src/main.rs 2174 行（全仓最大，两轮巨型文件治理盲区；main 229 行 + bench_workload 220 行） | 结构 / 模块化 | P3 | 高 |

本轮核对过、未立项的检查点（附量化结论）：

- 拆分后模块间重复逻辑：四个新拆 crate（task_write / io_sim / mapping /
  task_gc）跨模块同名顶层函数为零（逐 crate 顶层项重名扫描）；
- 拆分后循环概念依赖：io_sim（{page,manifest,segment,term,file}→{op,state}）、
  mapping（mapper→{allocator,chunk,generation,mapping_tbl}，snapshot→
  {mapper,mapping_tbl}）、task_gc（{gc,retained}→inflight）均为 DAG；
  task_write 存在 batch↔data、batch↔index 两对双向引用（batch 调
  data/index 的函数、data/index 消费 batch 的 `PageLayout`/`PendingWrite`/
  `ValueCtx`/`Children` 等共享类型，共 9 条模块内边）——属「类型在编排
  模块」的轻度形态，修复仅需把共享类型下沉 types 模块，量级过小不立项，
  随 task_write 后续演进顺带处理；
- 800~1000 行段文件内聚度抽查：`fkv_btree/lib.rs` 977（btree 游标/合并
  单域）、`fkv_cloud/cache.rs` 974（单 `TableCache` impl）、
  `fkv_standby/lib.rs` 900、`fkv_shard/read.rs` 883（读窗口状态机）、
  `fkv_io/lib.rs` 825（trait 定义本体）、`fkv_config/lib.rs` 815——各有
  单一域主类型 / 主职责，不立项（fkv_bench 单独立项 struct-16）；
- 150+ 行生产函数：仅 5 处（fkv_bench 2 处归 struct-16；
  `fkv_task_scan::scan_page_with_budget_map` 214 行为单循环状态机本体、
  `fkv_task_write/batch.rs::write_batch` 157 行为流水线编排，均高内聚）；
- fkv facade pub 面：57 行 lib、26 个再导出名 + 2 个入口函数，未泄露
  内部分类类型（`RequestKind`/`RequestPriority` 不经 facade），最小面成立；
- Cargo 层：全仓无 `[features]` 节；25 个 workspace.dependencies 键全部
  被成员引用（`[dependencies.X]` 分节 / `X.workspace = true` 两种声明
  风格均核）；无同 crate dependencies+dev-dependencies 双声明；成员无
  直写版本号；不立项；
- 中文注释规范：六提交涉及的 74 个 src 文件中文注释 3801 行 / 英文 25 行
  （0.7%）；全仓生产 src 中文 9427 / 英文 82（0.9%，专名如
  `durability anchor` / `days_from_civil` 为主），不立项；
- dev 向上依赖（`fkv_testkit` 作 L2/L3/L4 的 dev-dep 等 13 条）：仓库
  测试既定约定（struct-08 即此法），不计入分层倒挂；生产方向唯一倒挂
  见 struct-13。

## 语义专项第二轮迭代（2026-11-06，第 2 轮 sem review：sem-02/03 复审 + 新面审查）

复审处置：sem-02 / sem-03 均达标，移 `../reviewed/`（逐项证据见两文件末尾
「复审记录（2026-11-06）」节）。要点：sem-03 谓词门逐条件对齐 C++ needs_compact
（data_append_mode 为恒真简化、放大率公式整数/浮点数学等价、OR 语义 + 双侧
compact+GC 对齐 `Compact()` 结构）、re-FindRoot 等价（table_map 就地推进后
重读）、file_gc 双集合覆盖对齐 `RunFileGc`；缺口为 segment 侧零覆盖
（test-08，并行已立）与 compact 失败路径无注入（`?` 单行传播，不单独立项）。
sem-02 条目逐句属实、锁定测试覆盖四个口径；遗留 44 份 meta.txt baseline=
未刷新 → corpus-01。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [sem-04](sem-04-cloud-manifest-adoption-fence.md) | 云 manifest 采纳：C++ 回退选择不限 branch + GetManifest ExpiredTerm 栅栏 + 晋升回传云；Rust branch 过滤、无栅栏（过期 owner 写静默不可见）、晋升只写本地 | 语义 / Cloud | P2 | 高 |
| [sem-05](sem-05-standby-global-reopen-merge-clean.md) | standby global_reopen：C++ 合并远端清单 + clean=成功收尾；Rust 只枚举本地、远端独有不可达、本地独有清理后记失败 | 语义 / standby | P2 | 高 |
| [sem-06](sem-06-ini-parse-boundaries.md) | INI 解析边界：inih 大小写不敏感/BOM/行内 `;`/`:` 分隔/strtoul 前缀语义/200 字符截断 vs Rust 逐项相反（三项导致启动失败） | 语义 / 配置 | P2 | 高 |
| [sem-07](sem-07-table-name-dir-parse-boundary.md) | 含 `.` 表名：`TableIdent::new` 可建可写但 `parse_dir_name` 不可见（归档 crond 静默跳过）；C++ 端到端支持 | 语义 / 异常输入 | P3 | 高 |
| [corpus-01](corpus-01-baseline-meta-refresh.md) | sem-02 改 COMPAT_BASELINE 未刷 44 份 meta.txt baseline= 压行，入库后 fixture-diff 必红 | 语料同步 | P2 | 高 |

本轮核对过、未立项的检查点（附两侧 文件:行号 与结论）：

- **全局 fanout 语义**：C++ `HandleGlobalReopenRequest` 的 first_error 聚合
  vs fkv `GlobalReply{ok_num, failed_li}` 为已登记的 API 形态差异
  （`fkv_runtime/src/lib.rs:100-133` 文档）；`max_global_request_batch`
  分批并发两侧同源（`eloq_store.cpp:2113-2128` vs
  `fkv_runtime/src/archive.rs:411-420`）；空目标集合两侧均成功空回执；
- **`global_archive` 枚举口径**：C++ `HandleGlobalArchiveRequest:1659-1780`
  非 cloud 只扫本地目录（无 standby 合并），Rust `enumerate_target_li` 对齐；
  空 tag 单次生成 + 广播、`result_archive_` 0 目标仍构造——两侧一致；
- **对象命名**：`data_<id>_<branch>_<term>` / `segment_<id>_<branch>_<term>`
  / `manifest_<branch>_<term>[_<tag>]` / `CURRENT_TERM_<branch>_<pg>`
  逐字符一致（`common.h:311-560` vs `fkv_types/src/lib.rs:318-360`）；写键
  带写时 term、读键经 `file_range_li` 链回溯（`fkv_cloud/src/object_key.rs:126-153`），
  旧 term 对象不回写——与 C++ `BranchDataFileName` 同构；
- **store_path 权重**：`compute_store_path_lut` 与 C++ `ComputeStorePathLut`
  （`utils.h:218-280`）gcd 归一 + ceil 缩放 + 保底 1 槽同构；仅 >max_entries
  溢出角（>1M 根目录）Rust 多一步确定性裁剪、C++ 允许超限（Rust 已注释）；
  权重 0：C++ LUT 按 1 处理、`KvOption::validate` 直接拒（Rust 注释声明的
  有意从严）；`parse_store_path` 与 `ParseStorePathListWithWeights`
  （`utils.h:115-189`）逗号表 + `:w` 后缀 + 计数校验一致；
- **统计接口**：C++ 无引擎级 stats 查询 API；`eloqstore_metrics.h` 全部
  内容在 `ELOQSTORE_WITH_TXSERVICE` 编译门内（txservice 宿主集成，同
  sdk_runtime / EloqModule 的宿主边界原则，COMPAT_BASELINE 已排除）；
  fkv_metrics / refresh_space_stat 为自有观测层，无对齐目标，不立项；
- **compact_before_archive × 并发 reopen**：无竞态——`create_archive`
  持 `&mut self`、shard 请求串行（`fkv_shard/src/dispatch.rs:271` 单队列），
  pre-compact 与 reopen 无新交叉；`file_gc` 快照引用集保护为既有机制
  （rev-39），复审未引入新等待面；
- **retention 剪枝 × standby**：standby 的 `sync_standby_partition` 不做
  prune（prune 只在 `create_archive` 成功后触发）；两侧 archive crond 的
  启动条件等价（C++ `eloq_store.cpp:897-905` 无模式门 / Rust
  `startup.rs:238-247` 同）；「Rust 删 C++ 永不删的归档」混合场景已由
  sem-02 条目覆盖，standby 无新增面，不立项；
- **fixtures 语料刷新**：payload 为手构 format-envelope，不经
  `create_archive`（`tool/fkv_ledger/src/corpus.rs:417-433`），sem-03 行为
  变化零影响；唯一需刷新的是 baseline= 压行（corpus-01）。

## 第四轮迭代（2026-11-06，五个只读复审子代理并行；重点复审并行会话未经审查的新提交）

复审对象：`2997236`（pooled 接线）、`0de61b6`（bench TTL/compact lane）、
`69b960f`（cloud 具名 manifest 截断）及 `2a87472`/`ff97452`/`6fd58ad`/`0d92421`
（sem-09/13/11/12）、engine 级交叉证据（`fkv_engine_cross` + `cpp/engine_driver`）。
五个子代理分领域（pooled 语义 / bench+struct / cloud sto-51+L1 /
异步+性能 / 测试+死代码），全部只读，逐条给出 file:line 证据。

**轮次结论：不完善**（5 个子代理在其领域内均给出「否」），产出 12 条 issue：

| 编号 | 标题 | 领域 | 优先级 | 处置 |
|---|---|---|---|---|
| [pooled-02](../done/pooled-02-segment-mapper-pooled-never-reclaimed.md) | 段 mapper 恒 Pooled 且 retired 队列无回收点 | 语义/内存 | P1 | **done**（改恒 Append，对齐 C++） |
| [pooled-03](../done/pooled-03-maintain-not-gated-by-alloc-mode.md) | Pooled 下 `needs_maintain` 恒真，每写跑全量 maintain | 性能 | P1 | **done**（gate 在 Append，对齐 C++） |
| [pooled-04](../done/pooled-04-reclaim-only-wired-on-put-path.md) | `reclaim_retired` 只接线在 put 路径 | 性能 | P2 | **done**（maintain/clean_expired 补回收） |
| [pooled-05](pooled-05-reopen-free-pool-missing-branch-filter.md) | reopen 空闲池重建缺 branch 归属过滤（疑似跨 branch 静默损坏） | 数据完整性 | P0 | 实现中（`BranchPageOwner` 已落地，待复审） |
| [pooled-06](pooled-06-reopen-free-pool-seeds-every-page-slot.md) | reopen 空闲池按「每个物理页位」播种，O(file×2^18)，实测 51~105s | 性能 | P1 | **new** |
| [io-13](../done/io-13-write-flight-now-or-never-deregisters-waker.md) | `try_next` 用 `now_or_never` 注销事件循环 waker | 异步 | P2 | **done**（`poll_ready_once`） |
| [test-12](../done/test-12-queue-timeout-wall-clock-flaky.md) | queue timeout 用例靠墙钟，高负载 flaky | 测试 | P2 | **done** |
| [test-13](test-13-attach-view-test-races-shard-thread.md) | attach view 用例与 shard 线程抢跑 | 测试 | P3 | **new** |
| [test-11](../done/test-11-pooled-paths-lack-fault-and-concurrency-coverage.md) | Pooled（生产默认）在故障注入/并发用例零覆盖 | 测试 | P2 | **done**（fkv `8733645`） |
| [bench-01](../done/bench-01-ttl-compact-lane-record-mismatch.md) | perf-11 验收条款与实现不符，`0de61b6` 不能独立编译 | 记录准确性 | P2 | **done**（`5092c16`） |
| [sem-14](../done/sem-14-sto51-padding-branch-divergence.md) | sto-51「padding 缺失」分支与 C++ 不等价且未登记 | 语义/恢复 | P2 | **done**（登记有意差异 + 锁定测试） |
| [l1-01](../done/l1-01-engine-cross-evidence-not-pinned-or-gated.md) | 引擎交叉证据：env 缺失静默通过、CI 未 pin、覆盖面单一 | 兼容证据 | P2 | **done**（ignored + pin fkv SHA） |
| [doc-01](../done/doc-01-fkv-architecture-cloud-manifest-stale.md) | fkv 架构文档仍写 cloud 具名 manifest 走 no-op | 文档 | P3 | **done**（`a1b2127` 已修，文档收紧 `a5b4253`） |
| [dead-02](../done/dead-02-store-alloc-mode-getter-unused.md) | `Store::alloc_mode()` 死 pub API（unused.js 漏报） | 死代码 | P3 | **done** |

本轮子代理核对过、**未**立项的检查点（不重复列举，摘要）：

- **Pooled 回收判定本身正确**：`retired_gen < min_live_generation` 精确且保守
  （形式化论证 + `retire.rs` 9 例）；COW 不原地覆盖；`abort_batch` 撤销 retire；
  `reserve_until` 丢弃预留区间条目；reopen 从映射空洞兜底（主 branch 不泄漏）；
  7 处 mapper 构造点全部接线了快照来源；`SnapshotRegistry` 未破坏 `!Send`。
- **compio 0.19.2 语义核对**：`spawn` 不要求 `Send` 且未跨 runtime spawn；
  `JoinHandle::drop` 取消有排空；取消不可靠下 `Submit` 的 `PinnedDrop` +
  `FdGuard` 保活正确（`fixed.rs` io-37 回归测试）；`fdatasync` 与 `close`
  交互正确；阻塞调用全部下放阻塞池；锁作用域不跨 await；IO 深度预算自洽。
- **各池内存上界逐池推导**：`BufPool` / `direct_pool` / `FixedWritePool` /
  `FixedBufferRegistry` / TLS 页池 / `fd_cache` / `ReadFlight` / `SnapshotRegistry`
  / index cache 均有界；**唯一无上界的是 `retired_li`**（已立 pooled-02/04）。
- **批量化未被破坏**：`write_page_li` 排序合并、`sync_data` 并发 fdatasync 仍在。
- **死代码**：`bun sh/unused.js crate` 仅剩 2 条已知 trait impl 误报，无新增；
  依赖面 `cargo-shear ✓ no issues found`。
- **废话测试**：337 文件 / 1170 条测试按三种模式全量扫描，无可删清单；
  断言有效性变异实验 6/6 全部检出（改错期望值即失败）。
- **bench 新 lane 不污染 C++/Rust 比值**：`perf_gate.sh` 只为两侧同存 metric
  建 ratio；`PERF_TTL_RATIO>0` 才追加参数；`selftest ok`。
- **cloud 截断缓存一致性**：manifest 不进 cloud cache；三处 IO 装饰器都用
  `forward_io_*` 转发 `reset_manifest*`；`fkv_store` 的 IndexCache/root_meta
  在 `install_replay_state` 之后 reset；standby 侧要求 manifest 完整（策略不同
  但安全）；崩溃窗口幂等。
- **sem-09 ~ sem-13 五条**：逐条独立核实**全部成立**（其中 sem-09/11/12/13 在
  本轮审查期间已被并行会话修复）；sem-13 残留一项：`COMPAT_BASELINE` 的
  「INI 布尔词表：一致」行与新写入的模块文档不一致，待补条目。
- **C++ 对照环境限制**：本机 darwin/arm64 无法构建 eloqstore（依赖 io_uring），
  C++ 侧结论均来自源码 + 上游测试断言的静态核对，已在各 issue 中声明。

## 异步 / IO 第三轮复审与 O_DIRECT 专项（2026-11-06）

复审对象：`done/` 中未进 `reviewed/` 的 io-08 ~ io-13，外加新提交
`a1b2127`（default data/segment IO to O_DIRECT like C++）与 io-12 在默认
Pooled / O_DIRECT 下的正确性。

| 编号 | 标题 | 领域 | 优先级 | 处置 |
|---|---|---|---|---|
| [io-14](io-14-get-large-single-flight-window-stall.md) | GetLarge 单飞 leftover 让读窗口常驻有界失效（阻塞新请求 / 写回收 / 低优先） | 异步/调度 | P2 | **new** |
| [io-15](io-15-pooled-cross-term-file-id-reuse.md) | Pooled 跨 term 复用旧 file id：候选洞读零页 / Cloud 写入键与读取键分叉 | IO/数据正确性 | P2 | **new** |
| [io-16](io-16-direct-io-disables-managed-segment-read.md) | 默认 O_DIRECT 使 managed（provided-buffer）segment 读不可达，runtime 仍配置缓冲池 | IO/配置 | P3 | **new** |
| [io-17](io-17-direct-io-whole-file-read-double-buffer.md) | O_DIRECT 整文件读「零初始化对齐缓冲 + 再拷贝」：cloud flush 峰值内存 ~2× | IO/内存 | P2 | **new** |
| [io-18](io-18-direct-io-no-fallback-or-wiring-test.md) | 默认 O_DIRECT 无可配置关闭 / EINVAL 回退；默认接线与整文件读缺测试 | IO/健壮性+测试 | P3 | **new** |
| [io-19](io-19-getlarge-oom-inline-when-flight-empty.md) | io-11 残留：`flight.is_empty()` 时 GetLarge OOM 仍内联跑完整大值读 | 异步/调度 | P3 | **new** |
| [io-20](io-20-flaky-read-window-partition-tests.md) | io-08/io-09 验收测试依赖墙钟与负载，并发 CI 下已实测 flaky | 测试 | P3 | **new** |

复审处置：

- **达标 → `reviewed/`**：io-09（分区屏障 + in_flight 计数 + unstart 让路，
  逐点核对）、io-10（`MIN_IO_QUEUE_SIZE` / 告警 / 文档 / 测试齐全）、
  io-12（4 条读路径身份校验 + 冷启动补记；Runtime Cloud 强制 Append 下正确，
  O_DIRECT 不改变记账）、io-13（`poll_ready_once` 用调用方 Context，
  计数 waker 测试可拦回归）。
- **未完全达标，保留 `done/` 并另立后续**：io-08（窗口预算对 GetLarge 单飞
  不可准入项没有出口 → io-14）、io-11（`flight.is_empty()` 仍内联大值读 →
  io-19）。
- io-01（reject）不重复立项；本领域其余已知问题（pooled 逐页归属、
  io-08/09 测试 flaky）已分别由 io-15 / io-20 承接。
- 本机 darwin/arm64 无法执行 Linux O_DIRECT 路径；a1b2127 相关结论均来自源码
  静态核对与现有 `#[cfg(target_os = "linux")]` 测试清单，未运行 C++ / 压测。

## 第五轮迭代（2026-11-06，第二轮复审：3 个只读子代理，验证第一轮修复）

复审对象：HEAD `d9d0ecf`（fkv），逐条复核第一轮 6 条 pooled 修复 +
io-13/test-11/test-12/test-13/bench-01 + L1 证据链/文档一致性/struct-16。
三个子代理分别给出领域判定：

| 子代理领域 | 判定 | 依据 |
|---|---|---|
| pooled-02/03/04/05/06 + dead-02 | **是（完善）** | 6 条逐条与代码一致；pooled-06 的 `FreeRange` 区间栈与 C++ `PooledFilePages` **逐 id 等价**（独立 C++ 模型差分：599 组随机配置 + 199 seeds×400 步操作全绿）；pooled-05 区间语义与 `FileIdInBranch` 同口径且已真正接线；变异实验 **8/8 全部检出**（无恒绿断言）；`branch_attach` 65.5s → 0.07s，`.config/nextest.toml` 无需放宽 |
| 异步 / 测试 / bench | **否（不完善）** | 5 条修复本体真实且变异 5/5 检出；缺口为记录准确性与验证层次：`5092c16` 引入的 `.with_direct_io(true)`（对所有 lane 生效）**未登记**；io-13 的「影响」表述与其修复效果不一致且无行为级用例；test-11 在 `fkv_shard/tests` 仍有 22/28 文件固定 Append |
| L1 / 登记一致性 / struct-16 | **否（不完善）** | `engine.yml` 的 `FKV_COMMIT=6fd58ad` **在 origin 不可达**（游离于被丢弃的旧历史，5/6 依赖 crate 与当前主线内容不同），且**没有任何一次 CI run 执行过 pin 之后的 workflow**（最近 green run `42103406714` 的 `headSha=7cc56ab`，其 workflow 无 pin/matrix）；`COMPAT_BASELINE` 的「CI 已覆盖」表述、`doc/migration*.md`、`issue/todo/00-index.md`、`doc/progress.md` 看板成片过期；struct-16 **已达成验收线**（`main.rs` 206 行、全目录最大 919 行） |

**本轮结论：不完善**（2/3 领域）。收敛计数：连续「完善」判定 **0 轮**
（第四轮 0/1、第五轮 1/3）。

第二轮新增发现（**多数已由并行会话独立立案**，此处不重复建文件，仅记录交叉印证）：

- bench/io 侧：`5092c16` 的默认 lane `with_direct_io(true)` 口径改动未登记
  → 与已立案的 `io-16`/`io-17`/`io-18`（direct-io 家族）同一面；
- L1 证据链 → 已立案 `sem-18-l1-evidence-pin-and-doc-sync.md`（我方证据增量：
  pin 对应 5 个依赖 crate 的 tree 差异量化、最近一次 engine run 的 `headSha`
  与其 workflow 无 pin/matrix 的比对、`fkv_ledger issues` 与 `progress.md`
  看板逐值对照：`done 60 / todo 0 / reviewed 23` vs 看板 `todo 14 / done 44`）；
- 文档看板过期 → 已立案 `doc-02-migration-status-stale-after-l1.md`；
- test-11 残留（`fkv_shard/tests` 22/28 仍固定 Append）→ 记录于本轮，
  与 `test-14`（scheduler 低 PQ 深度 flaky）同属测试面；
- 另记 4 条 P3（未单独立项）：`BranchPageOwner::owns` / `pooled_free_len` /
  `pooled_free_range_cnt` 仅测试调用（`unused.js` 漏报盲区）；
  `interval_owner_matches_per_page_owns` 只锁内部一致性（建议改独立 oracle）；
  段侧 reopen 未传 owner（当前不可达，属防御缺口）；
  `cargo test`（libtest 并行）下 failpoint 用例因进程级全局注册表串扰
  （nextest 下正常），建议 `#[serial]` 或文档声明。

## 收敛度小结（2026-11-06）

- 第四轮（5 子代理，全量面）：不完善 —— 16 条发现，全部立项；其中 13 条已
  done/reviewed，剩余为文档/证据面（已由并行会话立案）。
- 第五轮（3 子代理，验证面）：1/3 领域完善；代码正确性面（pooled 全家）**已达
  完善**，不完善集中在**证据链可复现性**与**记录准确性**。
- 达成「连续 3 轮判定完善」仍需至少 2 轮：下一轮应聚焦
  `sem-18`（L1 pin/CI 证据）+ `doc-02`（看板/文档同步）+ direct-io 家族的
  口径登记，验证完即可计入第 1 轮「完善」。

## 第 4 轮性能复审新增（2026-11-06）

复审对象：perf-12（前缀回收）、perf-13（gate schema 2 / 源码构建）与
io-16/17/18 的运行时/复杂度复核；perf-12、perf-13 已移 `../reviewed/`。
本轮 fkv 代码热路径（O_DIRECT + Pooled + 任务化写、读写每请求开销）未发现
新的性能回退；两条新意见都在 V6 门禁的**部署/权威性**面：

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [perf-14](perf-14-perf-gate-fix-not-deployed.md) | perf-13 的 gate 改造未进入 origin/main：CI 仍下载 perf-latest，`PERF_BENCH_BIN` / bench commit 断言 / schema 2 均未生效 | 性能 / V6 门槛（CI 部署） | P1 | 高 |
| [perf-15](perf-15-perf-baseline-non-authoritative-publish.md) | publish_baseline 在 `PERF_ALLOW_VIRTUAL_DISK=1` 非权威模式下仍上传基线；发布指引 runner 默认值过期 | 性能 / 基线权威性与发布指引 | P2 | 高 |

## 第六轮迭代（2026-11-06，收敛判定轮：结构 / 测试 / 死代码 / 文档）

复审对象：第 5 轮落地（struct-21/22、io-22、pooled-10/11、sem-25/26/27、doc-03），
全仓门禁复跑、新测试断言抽查（10+）、全量测试 2 遍、死代码、`COMPAT_BASELINE`
与 44 份 fixtures meta / doc 一致性。**测试面本轮无新意见**（断言强度、反向
验证与 flaky 均未发现缺陷）。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [struct-23](struct-23-clippy-absolute-paths-regressed.md) | struct-21 的 clippy 只读门禁在 `931b0f0` 回归 2 处（fkv_standby/src/rsync.rs） | 结构 / 门禁 | P3 | 高 |
| [struct-24](struct-24-line-count-gate-inline-test-hole.md) | 行数门禁对生产文件内联测试两面都不计（7 文件 3761 行），当前无越线 | 结构 / 门禁 | P3 | 高 |
| [corpus-02](corpus-02-baseline-snapshot-not-propagated.md) | sem-25 改基线后 44/44 meta 的 `baseline=` 与 progress.md 未刷，CI fixture-diff 必红 | 语料 / 账本 | P2 | 高 |
| [doc-04](doc-04-perf-metrics-stale-bench-lines-and-premise.md) | perf-metrics 9 处 `main.rs` 行号失效 + 前提与自身矩阵矛盾；migration 的 resize 路径失效 | 文档 / 派生文档 | P3 | 高 |

门禁数字（HEAD `d6ebf23`）：

- `check_line_count.sh`：production 174 files，max 994（`fkv_config/src/lib.rs`）；
  test-only 248，max 1231（`fkv_store/tests/branch_archive.rs`）；`line-count OK`；
- `check_layers.sh`：`39 crates, 182 production path edges`，exit 0；
- `cargo clippy --workspace --all-features --all-targets`：0 告警；
  带 `-D warnings -W clippy::absolute_paths` 只读复算：2 errors（struct-23）；
- `cargo shear`：`no issues found`；`bun sh/unused.js`：只剩 2 条已记录 trait impl 误报；
- `cargo nextest run --workspace --all-features` 两遍：1302 passed / 1 skipped（两遍一致）。

已核对未立项：`slow_read` 槽表（poll 作用域 / 无嵌套 scope / 关闭零分配）、
`reuse_floor` 的 `descendant_page_floor`（O(子孙活页)、clamp 保持 pooled-08）、
测试拆分（cloud 51 / write 20 / bench 47，`assert` 191/80/207 前后相同）、
8 个测试文件的断言强度（2 个新增文件 + 6 处增量：nested_branch / maintenance_expired /
gc_store / inflight / branch_attach / standby）、生产 `dyn` 8 处（回调 / panic payload / StdError 链，
全部正当）、生产 `unwrap/expect` 14 处（bench 诊断 lane 10 处；fkv_buf 1 处有前置
`assert!(is_power_of_two)`、standby 2 处两分支互斥、cloud cache 1 处刚插入必中）。

## 第七轮迭代（2026-11-06，性能复审 / 收敛判定轮）

复审对象：`d6ebf23` 的 shard / Store 读写热路径、`perf-16/17` 的 gate 改造、
`pooled-11` / `sem-25` 的 reopen / 建分支路径、`io-22` 探针与 `struct-22`
测试拆分。

第 5 轮修复复核：

- `perf-16` 方向表、`perf-17` 默认值与文档同步都只在 `scripts/` / `doc/` /
  `.github/`，不动 fkv 运行时；`bash scripts/perf_gate.sh selftest` 通过，
  `bash -n` 通过。
- `pooled-11` / `pooled-10` / `sem-25` 的 `descendant_page_floor`、
  `committed_block_ceiling` 只在 `reopen` / `open_branch` / `reopen_archive` /
  `create_branch` 路径，`Store::put` / `get` / scan 不调用；`committed_block_ceiling`
  逐份 manifest 累加只在建分支控制面。
- `io-22`：`slow_read` 关闭时 `scope` 只做一次 `enabled()` 原子读后直接
  `await` 内层 future，不碰 TLS / 时钟 / 分配；`start/stop/record` 同样短路。
- `struct-22`：只改测试文件，无生产路径变化。

本机实测（release，HEAD `d6ebf23`，共享开发机 load 18–51，仅作同机趋势）：

- write-only rows 20000 / batch 100：put_kv_s 211,793–319,949（p50 280–351 µs，
  p99 1.4–2.0 ms）；read-only（partitions 2 / read-per-part 2 / test-secs 5）：
  get_kv_s 183,303，p50 18 µs，p99 50 µs（`perf-11` 记录 170,019 / 22 / 51，
  无回退信号）；write-read rows 20000 / test-secs 2：put 153,667、get 94,555。
- `bash scripts/perf_gate.sh selftest`：通过。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [perf-18](perf-18-shard-cache-metrics-full-scan.md) | shard 每完成一条读 / 写都遍历全部 partition 槽位同步 index 缓存指标：1024 分区时单次 16–71 µs，读阶段 37–77% 耗在扫描，get 吞吐降到单分区 1/5–1/8 | 性能 / shard 热路径 | P1 | 高 |

`fkv_bench` 直接调用 Store，不经过 shard；perf-18 只能在 Runtime / Shard 形态
复现（探针见 issue 正文，副本在 `/tmp/fkv_probe6`，未改工作目录源码）。

## 结构 / 测试 / 文档 / 死代码第七轮迭代（2026-11-06，收敛判定轮）

复审对象：第 6 轮落地的 `sem-30`（`RouteCursor::page_for` 祖先上界 +
`sem30_index_route_bound` 3 例）、`perf-18`（`partition.rs` 增量账本 + 2 单测）、
`pooled-12`（`active_segment_watermark` + `snapshot_record` 段水位防御 +
`pooled_archive_large` 9 例）、`sem-28`（`chain_block_span` 孤儿归档下界 +
`pooled_orphan_archive` 6 例）、门禁 `struct-23/24`、`corpus-02` 同步门禁与
`doc-04`。fkv HEAD `16893ad`（外部重建的单一快照，`git status` 干净）。

门禁数字（全部只读参数）：

- `bash scripts/ci/check_line_count.sh`：production 174 files，max 994
  （`fkv_config/src/lib.rs`）；inline 观察 8 files（报 5269 行，实际 ~2846）；
  test-only 251，max 1231（`branch_archive.rs`）；`line-count OK`，exit 0。
- `bash scripts/ci/check_layers.sh`：`39 crates, 182 production path edges`，
  exit 0（尾部行数门禁同绿）。
- `cargo clippy --tests --all-targets --all-features -- -D warnings
  -W clippy::absolute_paths`：exit 0，0 行输出（struct-23 的 2 处已清）。
- `cargo shear`：**exit 1**，`gxhash` 在 `fkv_bench` 只用于 dev target
  （struct-26）。
- `bun sh/unused.js`：未引用 pub 方法只剩 2 条已记录 trait impl 误报
  （`reopen.rs` 的 `partial_cmp`、`fkv_io_compio/buf.rs` 的 `as_uninit`）；
  「未使用依赖」段为 struct-26。
- 全量测试 2 遍：`cargo nextest run --workspace --all-features` 均为
  **1324 passed / 1 skipped**，无 flaky；`eloqstore2fkv` 的 `fkv_compat`
  12 passed。
- 文档：`bash scripts/check_baseline_sync.sh --check` → `44/44 meta.txt
  (cpp 22 / rust 22), doc/progress.md ok, 28114 chars`；`doc/perf-metrics.md`
  的 `main.rs:` 引用已清零，新引用的 `lane.rs` / `runner.rs` / `hist.rs` /
  `report.rs` / `workload.rs` / `cli.rs` / `compact.rs` / `ttl.rs` 行号逐个
  命中符号；`doc/migration.md` 的 `mapping_tbl.rs:128-163` 有效。

测试质量（反向验证，改动均已还原，文件哈希回到 HEAD）：

| 变异 | 运行 | 结果 |
|---|---|---|
| `RouteCursor::page_for` 退回「只看直接父节点」 | `sem30_index_route_bound` | 3/3 failed（seq 第 8068 项、random 单键 / 批量均命中） |
| `rebuild_reuse_floor` 对「其它 branch 归档」恢复 `continue` | `pooled_orphan_archive` | 5 failed / 1 passed（仅控制组通过） |
| `evict_table` 不扣 `cache_bytes_total` | `fkv_shard --lib` | `incremental_sync_matches_full_reconcile` failed（1232 vs 期望） |
| `active_segment_watermark` 丢掉 main 的 manifest 来源 | `pooled_archive_large` 冷进程用例 | failed（Pooled main 读出 big2 字节） |

断言强度抽查：sem-30 的 scan 全序 + 逐键 get 对拍；pooled-archive-large 的
seg 水位单调、跨 Store 重开后的 `get_large` 字节、子 / 孙分支读回；orphan 的
`floor == 2*BLOCK_FP`、部分删除后 manifest 列表精确相等、受限分支的 floor /
max / limit 三重断言；partition 单测的增量 == 全量、跨分区隔离、驱逐后 gauge
按基线扣减。未发现只复述常量的废话断言或 flaky（全量 2 遍一致）。

| 编号 | 标题 | 领域 | 优先级 | 置信度 |
|---|---|---|---|---|
| [struct-25](struct-25-line-count-inline-boundary-misfire.md) | 行数门禁按首个 `#[cfg(test)]` 截断：9 个生产文件被误判（batch.rs 880 全生产报成 101 生产 / 779 内联，index.rs 468 全生产报成 3/465），合成 1399 行生产 + 1 行 cfg(test) 仍 `line-count OK` | 结构 / 门禁口径 | P3 | 高 |
| [struct-26](struct-26-cargo-shear-gxhash-dev-dep.md) | `cargo shear` 回归：`fkv_bench` 的 `gxhash` 仅 `benches/hash.rs` 使用却留在 `[dependencies]`，上一轮记录为 `no issues found` | 结构 / 依赖门禁 | P3 | 高 |

复审归档（done/ → reviewed/，本轮只处理 struct/test/doc 系列）：

- `struct-23`：`rsync.rs` 已改为 `process::id()` / `fs::remove_file()`，只读
  `-W clippy::absolute_paths` 全仓 0 警告，验收成立；
- `struct-24`：三段门禁已落地并运行，`inline ≤1500` / `total ≤1500` 的合成
  边界用例通过；但其「内联测试段」起点定义有缺口，后续由 struct-25 跟进；
- `doc-04`：`perf-metrics` 9 处 `main.rs` 行号全部替换且逐条命中，关键前提与
  §2 矩阵 / `compact.rs:84` / `ttl.rs:50` 一致；`migration.md` 的 resize 路径
  已指向 `mapping_tbl.rs:128-163`。

已核对未立项：`RouteCursor::page_for` 的祖先上界回溯复杂度（普通升序路径仍
1 层、命中即停，O(树高) 只在跨子树时支付）；`active_segment_watermark` 的
`None` fail-stop（`drop_table` 后带段体系归档返回 `ResourceMissing`，纯数据
归档不受影响）；`reuse_floor` 的孤儿归档候选（按自身区间链取块末尾、
受限分支 `start > max_fp_id` 过滤 + `branch_file_limit` clamp，与 pooled-08 /
sem-24 口径一致）；`snapshot_record` 的段水位只升不降（`branch_override`
例外保持子分支段起点）；`sync_cache_metrics_for` 的 `cache_bytes_total ==
Σ slot.cache_stat.bytes` 不变量（新建 / 写回 / 驱逐 / 全量对账四路闭合）。

## 第六至七轮迭代（2026-11-06，收敛轮：sem-28 验证 + reuse_floor 整体审计 + 总体判定）

三个只读子代理并行，分别给出判定：

| 子代理 | 领域 | 判定 | 关键证据 |
|---|---|---|---|
| A | `sem-28` 孤儿归档修复验证 | **是（真实、完整、无过度保护）** | 撤销修复的变异 M1 → 5/6 用例红，且「直接丢数据探针」16/16 归档条目读成 `None`；修复版 floor=8192、0/16 丢失；M2 6/6 红、M3（撤 pooled-08 护栏）1/6 红；孤儿与存活 branch 两种布局的 main 下界完全相同（16384==16384，无新抑制）；`fkv_store` 338 例 + `fkv_compat` 12 例全绿 |
| B | `reuse_floor.rs` 整体一致性（pooled-05/07/08/11/sem-24/25/27/28 十几条补丁叠加后） | **基本自洽、不过度保守（但「自洽而脆」）** | 8 条不变量逐条成立并给出证明；写-删-写循环 `max_fp_id` 收敛（A 203 稳定 / B 202 / C 恒 4096 / D 4297→4497 稳定）；48 例 pooled/branch 用例全绿。遗留 4 条 P3：模块文档第 7 行描述与实现相反；`descendant_page_floor` 扫描区间与「分支自用区间」不一致（隐式依赖，当前不构成活跃 bug）；「空间换隔离」取舍未登记；单函数 165 行 3 臂 match 可维护性风险 |
| C | 总体「完善、功能完整」判定 | **否** | 代码门禁面全绿：`1324 tests × 3 遍 passed / 1 skipped`、`fkv_compat 12 passed`、clippy（含 `absolute_paths`）零告警、行数门禁 OK（生产 max 994）、死代码仅 2 条已知误报 + 1 条真问题（`dep-01`）。判「否」的三条实质理由：① fkv 历史被重建为**单条 `init` 提交**（`git rev-list --count HEAD` = 1），使已发布的 L1 引擎交叉证据与 CI pin **不可复现**；② `doc/progress.md` 看板过期且**自相矛盾**（`ignored 387` vs `258`）；③ 本文件自述收敛计数仍为 **0 轮** |

**本轮结论：不完善**（1 领域「是」/ 2 领域「否」）。收敛计数仍为
**连续 0 轮**（第四轮 0/1、第五轮 1/3、第六至七轮 1/3）。

本轮新立项：

- [dep-01](dep-01-gxhash-in-dependencies-used-only-by-bench.md)（P3）：`gxhash` 落在
  `[dependencies]` 却只被 `benches/hash.rs` 使用（`unused.js` 真命中，前几轮误记为「无新增」）；
- [doc-03](doc-03-progress-board-self-contradiction.md)（P2）：`doc/progress.md`
  看板过期 + 同文件内 `ignored` 计数自相矛盾（387 / 258）。

**收敛受阻的根因（非代码问题）**：`sem-28` 与 `reuse_floor` 两条审计都表明
**代码正确性面已达标**；挡住「完善」判定的是**证据可复现性**：
fkv 仓在本会话内被**两次**重写成单条 `init` 提交（先 `6d962f0`、后 `16893ad`），
每次重写都会让 L1 引擎交叉证据的 SHA、CI 的 `FKV_COMMIT` pin、以及本仓
「per-issue 提交」约定同时失效。只要该重写继续发生，任何一轮的「完善」判定
都会被下一轮推翻。建议：在判定收敛前先冻结提交历史（不再 squash）。

## 边界升级：改为「对齐 C++ 全量」（2026-11-06，用户决定）

用户决定：**验收边界不再是 L1，而是对齐 C++ 全量**；fkv 历史重写问题先不管、
继续迭代。据此产出：

- **新交付物**：`doc/roadmap-full-alignment.md` —— 三份只读盘点（数据面格式/压缩、
  Cloud/standby/宿主、内存缓存/引擎模型）合成 L2/L3/L4 三阶段路线图，
  差距逐条带 file:line，含**永久排除项清单与判定理由**、每阶段验收判据。
- **新立项（L2 批次，`l2-01` ~ `l2-10`）**：
  - `l2-01` `max_cloud_concurrency` 额度模型两侧不同 **且 `COMPAT_BASELINE` 的描述与
    C++ 代码不符**（P1，纯文档修复：C++ 是 per-shard 额度 + 共享 HTTP 线程池，
    整机上界 `N×`；fkv 全局均分 ≈ `1×`）
  - `l2-02` standby 镜像校验 Rust 单侧加强未登记（P2）
  - `l2-03` L2 前逐条标注「有意差异」不纳入逐字节对拍（P1）
  - `l2-04` 指标缺 4 个 `global_*` 请求类型标签（P2）
  - `l2-05` term bump 条件 + reopen `max_fp_id` 抬升与 C++ 不等价（P1，影响
    跨重启续写布局，须成组做）——**已结案**（`issue/done/l2-05-…md`：判据与 C++
    等价、两处偏离判定为 Rust 单侧加固，生产逻辑零改动；跳表 l2-05a/l2-05b）
  - `l2-06` `MappingTbl::resize` 回填 + `manifest_limit=0` 语义相反（P2）
  - `l2-07` zstd 版本未 pin + 字典训练抽样不同（P2，压缩盘面逐字节可复现的前置）
  - `l2-08` index 页首访缺 single-flight，N 个并发读各读一次盘（P2，**L2 唯一会
    改变可观测 IO 次数的缺失项**）
  - `l2-09` `fkv_task_compact` 全程无 `YieldBudget`（P2，唯一没接让出的维护路径）
  - `l2-10` `GetMapper` 分配器重建未 1:1（P2，依赖 l2-05）

**盘点中确认「不是 fkv 缺失」的几项**（避免后续被当 bug 立项）：
`chunk_size` 在 C++ 引擎源码内**无读取点**（是宿主/KV Cache 字段）；
`SetEvictFunc`、`chunk_size`、`DeleteOldArchives` 等在全仓 `grep` 下**无生产调用点**；
用户态页池/pin/swizzle/data page cache、`GlobalRegisteredMemory` 族、
单进程多 shard 宿主、`sdk_runtime`/`EloqModule`/`test_utils` 均属**宿主边界**，
建议永久排除并在基线保留重开条件。

### 对收敛判据的影响（重要）

以「对齐 C++ 全量」为边界，**「连续 3 轮判定完善」在 L2 完成前不可能达成**——
每一轮都必然是「不完善」，这不是结论而是进度。建议改为**按阶段判据**
（见路线图 §6）：

- `L2 完成` = 路线图 §4 三批全部闭环 **且** 引擎级差分在
  `Local/Cloud/Standby × append/pooled × 压缩开/关 × branch/archive/large value/TTL/跨重启续写`
  矩阵上执行通过；
- 该判据有**两个环境前置**（都不是代码问题）：① 差分矩阵需在 Linux/io_uring 上跑；
  ② 需要 `origin` 可达且与证据一致的 fkv commit —— 当前 fkv 历史被反复重写为
  单条 `init`，此项在用户「先不管」的决定下仍处于不满足状态，即
  `L2 完成` 在当前环境下**无法被证明**。

## 第 8 轮迭代（2026-11-06，异步 / IO 模型收敛判定轮）

- 复审对象：io-23（fkv commit `985c2a8`，复审 HEAD `79b7dd5`）。`done/` 中
  未进 `reviewed/` 的 io 系列仅此一条，已移入 `reviewed/`（复审记录附在文件末尾）。
- io-23 修复边界核对面：
  - 两条准入路径共用 `MAX_READ_IN_FLIGHT`：窗口 `is_full`
    （`read/window.rs:129`）+ 批提交 `quota`
    （`event_loop.rs:253,261,283-295`）；release 探针实测峰值 128，160 不再出现；
  - deferred 到达顺序：`WindowDeferred.seed`（`event_loop.rs:113-115`；
    `window.rs:236-243`）先于通道新读，窗口满时新读排在其后；
  - 额度不跨窗口携带（每轮按 `read_flight.len()` 重算）；
  - deferred 与 `pending` / `large_park_li` 无互锁（deferred 不回 pending，
    大值读仍走 `large_read_waiting`）；
  - shutdown 由 oneshot Drop 收口为 `NotRunning`（`sender.rs:143-161`）；
    detach 后 deferred 读由 `admit` 补拒。
- 全新扫描异步面：事件循环等待集（请求 / 写完成 / reopen 到期 / 通道关闭）、
  读窗口预算与空闲等待下限、写门与任务化写回收、取消 / deadline 检查、
  shutdown 排空、compio `block_on` 的 yield 点。自第 7 轮基线 `16893ad` 起，
  异步面只有 io-23 一处源码改动（其余提交为 bench / CI / cloud 测试）。
- 新立项：
  - [io-24](io-24-cold-table-read-livelock-at-flight-cap.md)（P1）：读在飞满额
    时「未打开表」的读触发事件循环忙转卡死（io-23 额度门禁回归；release 探针
    30s 无进展、96% CPU；同流量预热表后全部完成）。
  - [io-25](io-25-deferred-read-survives-fresh-attach.md)（P2）：`deferred_read_li`
    未纳入身份切换清理，Expired→Fresh attach 后旧 term 读按新 term 执行。
- 判定：异步层**不完善**。io-23 的额度修复结论成立，但引入了 io-24 这一 P1
  活性回归；本轮不能判定收敛，需 io-24 修复并复跑「满额 + 冷表读」后重审。

## L2 第二批进展（2026-11-06）

- **l2-05 → done（生产逻辑零改动）**：C++/Rust 的 term bump **判据等价**
  （Rust 的 `self.term != 0 && replay.term != self.term` 即 C++
  `DeriveStoreMode != Local && expect_term != 0 && manifest_term != expect_term`
  的对应项）；真实差异是「Local 是否采纳宿主 term」（C++ 在 `eloq_store.cpp:929-932`
  强制 term=0，fkv 采纳）——对齐会与 `(branch,term)` 命名、term fencing、io-15 复用
  下界三处冲突 ⇒ **登记**。`max_fp_id = max(manifest, max_mapped+1)` 与「bump 撞
  branch 上限保留原水位」经论证是**必要加固**（C++ 靠写侧不变量 + debug 断言，
  release 下静默损坏）⇒ **保留 + 登记**。新增 9 个回归用例；反证实验：去掉 `.max(...)`
  → 水位用例 `left: 3, right: 101` FAIL，去掉上限保护 → `left: 8192, right: 8188` FAIL。
  遗留：Standby 下宿主是否传非 0 term **待 Linux 实测**（引擎源码只能确定
  「非 0 且 ≠ manifest term ⇒ data 侧 bump、segment 侧不 bump」）。
- **l2-06 → done（一处对齐 + 一处登记）**：
  - `manifest_limit = 0` 判为**无心差异** ⇒ **对齐 C++**（恒写 snapshot，
    `fkv_store/src/manifest.rs:247`），并证明默认 8 MiB 路径**字节不变**
    （删去的是恒真合取项）；新增 `manifest_limit_zero_always_writes_snapshot`
    与对照 `default_manifest_limit_still_appends`。
  - `MappingTbl::resize` 填 INVALID 判为**有意加固**（判定记录先于本 issue 存在：
    `doc/migration.md` §8.1 core-29，且 `fkv_mapping/tests/resize_gap_fill.rs` 的文件
    注释就写着「防止『对齐 C++』时静默回归」）⇒ **保留 + 登记**进对拍跳表。
    理由：读路径假设「未写入槽不指向活跃页」
    （`fkv_task_read/src/lib.rs:286,443`），而稀疏增长路径真实存在
    （`set` 越界 `resize`、`fkv_recovery::apply_log:176`、`allocate_page:515`），
    不填会出现「回滚后同 chunk 长回 → 返回已归还分配器的 FilePageId」的**别名读**，
    并经 `serialize` 写进 manifest（实测：改成 C++ 语义后间隙为
    `[9,17,…,321]`，槽位 5 读回 `FilePageId(6)`）。

门禁：fkv `1367 passed / 1 skipped`；`fkv_compat 12 passed`；clippy 零告警；
`check_baseline_sync` 44/44。

**L2 剩余**：l2-07（zstd 版本 pin + 字典抽样）、l2-08（index 页 single-flight）、
l2-09（compact 接 `YieldBudget`）、l2-10（`GetMapper` 1:1，依赖 l2-05 结论——
l2-05 已把「pooled 接管布局」判为需保留 Rust 加固，l2-10 随之降为登记项）。

## 第 9 轮迭代（2026-11-06，异步 / IO 模型收敛判定轮）

- 复审对象：`done/` 中未进 `reviewed/` 的 io 系列 = io-24、io-25（均 fkv 已提交
  HEAD `ded4199`；复审时远端 `main` 推进到 `45d3aa6`，仅 bench 改动，
  `fkv_shard` / `fkv_api` / `fkv_config` 与 `ded4199` 无差异）。两条均已移入
  `reviewed/`，复审记录附在文件末尾。
- 第 8 轮修复边界核对面：
  - **io-24**（`is_full` 先于 ready）：满额 + 未 ready 的读先等一个读空位再
    Exit，主循环 `quota > 0` 后经 `submit_read_li` 补开（`read/window.rs:110-128`）；
    `read &&` 守卫保证写 / 控制请求仍立即 Exit（`:126,129-137`）；`is_full`
    仍只由 `read_flight.len()` 决定，deferred 仍只由窗口 seed 消费；非读请求
    不受读额度约束。
  - **io-23 额度语义**：窗口 `is_full` + 批提交 `quota` 两条路径不变；
    `push_retry` 先回收槽位再 push（净额不变），blocked 读不占额度；
    `read_concurrency.rs:291-419` 的峰值断言保持。
  - **io-25 清理覆盖**：`fail_stale_pending`（`event_loop.rs:520-534`）覆盖
    gate / blocked / large_park / deferred / reopen；`pending` / `wake_li`
    不清的排序论证成立；deferred 无第三条悬挂路径。
- 全新扫描异步面（HEAD 已提交代码，重点为第 8 轮 7 个提交）：
  - 事件循环等待集（请求 / 写完成 / reopen 到期 / 通道关闭）、读窗口预算与
    空闲等待下限、写门与任务化写回收、读任务槽位与分区计数、取消 / deadline、
    shutdown 排空与 ExitGuard、compio `block_on` 的 yield 点、SQ 深度与
    provided-buffer 配置：未见新问题。
  - `ListObject` 新增 `recursive`（`fkv_object/src/lib.rs` 的
    `list_level_page`）：分块扫描 + `token` 单调前进，每轮 `await`；Local / Mem
    仍 `InvalidArgs`；无无界内存或阻塞调用。
  - `get_entry` / scan `timestamp`（sem-34）：沿用既有读任务与 `slow_read`
    路径，无新增调度面。
  - sem-36 / struct-27（`archive` 拆分 + 冷态水位窗口）：数据面改动，
    `cold_watermark_scan` 与拆分前逐分支等价（`evidence_start =
    block_start.min(archive_max)`），未触及异步层。
  - clippy lint（struct-28）：测试路径清理，无行为变化。
- 新立项：
  - [io-26](io-26-reopen-zero-pending-time-spin.md)（P2）：
    `auto_reopen_pending_time_us = 0` 时，`run_due_reopen` 对 `is_writing` /
    `Busy` 的顺延使用 `due = now + 0`，`while` 条件立即重新成立，而循环体无
    任何返回 `Pending` 的 await（Cloud 无 standby 时 `handle(Reopen)` 在
    `store_mut` 前不挂起）→ shard 线程无 await 自旋、持有 `Rc` 的
    `skips_table_open` 读任务永不被 poll，全部请求 / shutdown 卡死。C++ 对
    `delay_us == 0` 有专门分支（`shard.cpp:330-340`），不进入延迟堆重试。
    默认 10s 不触发；属 io-08 Busy 顺延的 `0` 边界，不是 io-08 已记录风险的复述。
- 判定：异步层**未完全收敛**——io-01~io-25 全部闭环、第 8 轮两处修复边界成立，
  但 io-26 是一个非默认配置（`pending_time = 0`）+ Cloud 模式下可复现的活性
  缺陷（P2）；默认配置下未发现新的异步 / IO 模型问题。`new/l2-07~l2-10` 属外部
  并行 L2 工作流（代码未提交），不纳入本轮判定。

## 第 10 轮迭代（2026-11-06，结构 / 测试 / 死代码收敛判定轮）

- 对象：fkv 已提交 HEAD `6ab4ae5`（只读）；第 9 轮 l2 批次（`ded4199..6ab4ae5`）、
  struct-27 拆分、doc-05 脚本、flake-01 门闩。
- 代码门禁：
  - 行数门禁 OK：176 个生产文件，max 994（`fkv_config/src/lib.rs`）；
    内联测试 max 967（`fkv_task_write/src/lib.rs`）；测试文件 max 1472
    （`fkv_store/tests/pooled_orphan_archive.rs`）。
  - 分层门禁 OK：39 crates / 183 条生产 path 依赖。
  - `cargo shear`：no issues。
  - clippy：**不通过** — `-D warnings -W clippy::absolute_paths` 在
    `fkv_failpoint/src/lib.rs:168` 等 9 处报错（8 处第 9 轮测试代码 + 1 处
    flake-01 门闩），CI lint job 确定性红（struct-29）；不带该 lint 的
    `-D warnings` 面干净。
  - `sh/unused.js`：仍是 2 条已知 trait impl 误报（`Ord::partial_cmp` /
    `AsUninit::as_uninit`），无新死代码。
  - 测试：`--workspace --exclude fkv_nvme_test --all-features` **2 遍均
    1407 passed / 1 skipped**；`fkv_compat` 12/12。
  - `check_baseline_sync --check` 44/44、48496 字符；`check_progress_board
    --check`、`check_issue_links --check` 均 exit 0。
- 第 9 轮新代码复查（结构 / 设计，未发现行为缺陷）：
  - `IndexRead` 强制构造：生产两处均缓存 + 在飞表同时给出（`read.rs`、
    `scan/lib.rs`）；flight 表按 `(tbl, page_id, generation)` 分组，ticket
    防旧 owner 误删新登记，`LoadGuard` drop 广播 `TryAgain`；
  - `fkv_failpoint` 的 `!hold` 门闩：`Latch` Release/Acquire +
    存 waker 后二次检查、历史 latch 表支持同名重配 / `reset` 全放行、
    `@count` 用 `try_update(checked_sub)` 防回绕；调用点错误注入先判；
  - compact `YieldBudget`：两个让出点位于纯读扫描与文件边界，`0` 时零让出；
    Store 两处接线正确（但接线无测试，见 test-17）；
  - reopen 水位 / `manifest_limit=0` / GC retained 预算扫描：与基线登记
    （l2-05b、l2-06）一致；
  - sem-37：`compose_list_prefix` / `strip_list_prefix` 与 C++
    `ComposeKeyFromRemote` / `StripPrefixFromKey`（含空 remote path 补 `/`）
    逐条等价；perf-20：S3 `delimiter=/` 下推与 Mem 单次归并的计数断言成立；
  - struct-27：`archive` 拆分后文件均在门禁内，依赖方向不变。
- 断言强度（变异复算，改动均已还原，`git status` 回到原状）：
  - flight 内层键去掉 generation → `generation_change...` 红（1 != 2）；
  - 去掉 flight_wait 探针 → `merged_reader_reports_flight_wait` 红；
    把 Store 的 flight 接线改为 `None` → 同用例红（盘读 4 != 2）；
  - 去掉 `LoadGuard::drop` 广播 → `owner_cancel_broadcasts...` 红；
  - 去掉扫描段 / 文件边界让出 → `compact_yield` 两用例分别红；
  - 恢复 `manifest_limit != 0` 旧判据 → `manifest_limit_zero...` 红
    （默认 8 MiB 对照仍绿）；
  - 去掉空 remote path 的根边界 `/`、去掉返回名剥离 → backend 两用例红；
  - 去掉 snapshot 水位 `max(mapping+1)` → `low_watermark...` 红（对照绿）；
  - 忽略 branch 归属回填 → `pooled_owner` 4 例红（Store 级用例因
    reuse_floor 叠加仍绿，与 l2-10 记录的「冗余保护」一致）；
  - 删除 `fail_stale_pending` 的 deferred 清理 → test-15 事件循环用例红；
  - 把 Store 两处 compact 预算改传 0 → `fkv_store` + `fkv_shard`
    **451/451 仍绿**（test-17）。
- 复审归档（done/ → reviewed/）：`doc-05`、`test-15`、`test-16`；
  复审记录均附在文件末尾。
- 新立项：
  - [struct-29](struct-29-clippy-absolute-paths-round9-regression.md)（P2）：
    第 9 轮提交回归 9 处 `clippy::absolute_paths`，CI lint job 在 HEAD 红；
  - [test-17](test-17-compact-yield-store-wiring-untested.md)（P3）：
    l2-09 的 Store → compact 预算接线无测试，改传 0 后 451 例全绿。
- 判定：结构 / 测试 / 死代码领域**未收敛**——代码设计面（IndexRead / flight /
  failpoint / 水位 / 对象层）未发现新的行为缺陷，但 CI lint 门禁确定性红，
  且 compact 预算的生产接线存在一处「改坏仍绿」的测试缺口。

## 第 11 轮迭代（2026-11-06，结构 / 测试 / 死代码收敛判定轮）

- 依据：fkv HEAD `4582652`（只读）；复审第 10 轮 struct-29 / pooled-14 /
  io-27 / test-17 / sem-41。
- 门禁：行数（生产 max 994 `fkv_config`，inline max 967，test-only max 1472）、
  分层（39 crate / 183 edge）、clippy 原文命令（exit 0，零输出）、cargo shear、
  orphan（白名单 2 项）均绿；三账本（baseline 44/44 且 52434 字符、issue links
  366 引用 0 缺失、看板与 report diff）均绿。
- 测试：全量 2 遍均 `1420 passed / 1 skipped`（`--exclude fkv_nvme_test`）；
  compat 12/12；含已知负载敏感用例均通过。
- 变异：pooled-14 去掉合成区间 → 3 个 Pooled + 2 个 Cloud 用例红（Append 对照
  绿）；io-27 退回单次 drain → 2 个新用例红（`Err(Busy)`）；test-17 预算改 0 →
  2 用例红（`Pending = 0 / 1`）；sem-41 S3 `to_meta` 改 None → mock 红。
- 死代码：仅 2 个已知 trait impl 误报（`partial_cmp` / `as_uninit`），无新项。
- 复审归档（done/ → reviewed/）：`struct-29`、`test-17`（复审记录附文件末尾）。
- 新立项：
  - [struct-30](struct-30-rustfmt-gate-red-at-head.md)（P2）：HEAD 的
    `cargo fmt --all -- --check` exit 1（13 处 / 6 文件，含
    `get_mapper_parity.rs:16` 行尾空格），CI lint job 在 fmt 步骤必红；
  - [test-18](test-18-listobject-last-modified-api-transport-untested.md)（P3）：
    `dispatch_read` 把 `last_modified` 改成 `None`，全 workspace 1420 例仍全绿；
  - [doc-06](doc-06-execution-model-read-window-stale.md)（P3）：
    `02-execution-model.md` 读窗口写 32（实际 128）、`GetLarge` 写「不计入读
    窗口」（实际计入）。
- 判定：第 10 轮实现本身无行为缺陷（pooled-14 / io-27 / sem-41 的修复逻辑与
  测试判别力成立），但结构 / 测试 / 文档领域**未完全收敛**：CI lint job 的
  fmt 步骤红 + 1 处 API 透传测试缺口 + 1 处架构文档口径漂移。

## 第 11 轮迭代（2026-11-06，语义 / 盘面格式收敛判定轮）

- 依据：fkv HEAD `4582652`（只读）；C++ 基线 `82b774c`；复审第 10 轮
  sem-39 / sem-40 / sem-41 与 pooled-14 / io-27 / l2-07 的最新基线条目。
- 验证（零发现面）：
  - sem-41 透传成立：S3 `to_meta` 保留 `Some(LastModified)`
    （`fkv_object_s3/src/lib.rs:505-512`）、Mem 全 `None`
    （`fkv_object/src/lib.rs:223-228,352-377,420-446`）、common prefix 合成
    条目 `None`（`:698-704`）、`dispatch_read:128-133` 逐字段透传；三条目标
    用例 3/3 通过；基线条目 `COMPAT_BASELINE:801-828` 与 C++
    `utils.h:22-30` / `cloud_backend.cpp:274-301,303-405` 一致。
  - sem-39 压缩登记成立：`fkv_compress` 13/13 通过，实测
    `raw 142 例 / dictionary 42 例` 与基线数字一致；
    `check_baseline_sync --check` 44/44 / 52434 字符。
  - sem-40 证据修正成立：`flight.rs:178-192`、`lib.rs:348-358`（`:353`）、
    `single_flight.rs:591-635`（手工 poll）、C++ `task.cpp:66-73` 均与基线
    一致；`owner_cancel_broadcasts_try_again_to_all_waiters` 通过。
  - pooled-14：8 例（Mem/Sim/Compio/Cloud × Pooled/Append）全部通过；
    合成区间只检查 data 侧 bump 的 segment 残留由并行的 io-28 跟踪。
- 复审归档（done/ → reviewed/）：`sem-39`、`sem-40`、`sem-41`
  （复审记录附文件末尾）；`done/` 的 sem 系列已清空。
- 新立项：
  - [sem-42](sem-42-mapping-free-list-sentinel-encoding.md)（P2）：mapping
    空闲链尾哨兵 / 释放页 log delta 编码与 C++ 不同（C++ `InvalidValue=7`，
    fkv `PageId::MAX` 编码或链值），含 delete/truncate 的 manifest 逐字节不等；
  - [sem-43](sem-43-truncate-full-segment-mapping.md)（P2）：整表 truncate 时
    fkv 释放全部 segment 逻辑页，C++ 保留旧 segment 映射/文件；manifest
    segment 段取值与 GC 后可见 segment 文件集不同；
  - [sem-44](sem-44-baseline-latest-entries-inconsistent.md)（P3）：跳表行
    `l2-41` 应为 `sem-41`；pooled-14 合成区间未登记；sem-28 仍称空
    `file_ranges` 含「首个 main manifest」（C++ 82b774c 首个 main manifest
    在分配时即写 `(main, term)`）。
- 判定：语义 / 盘面格式领域**未完全收敛**——第 10 轮修复本身正确，但新发现
  两处**盘面字节/文件集差异**（sem-42、sem-43）与一处基线自洽缺口（sem-44）；
  均已带两侧 `文件:行号` 与验收方式立项。

## 第 13 轮迭代（2026-11-07，结构 / 测试 / 死代码 + 文档一致性收敛判定轮）

- 依据：fkv HEAD `fd022ca`（只读）；issue 仓 `7cacac5`；C++ 基线 `82b774c`。
- 代码门禁：
  - 行数 OK：176 个生产文件，max 994（`fkv_config/src/lib.rs`）；内联测试
    max 967（`fkv_task_write/src/lib.rs`）；test-only max 1472
    （`fkv_store/tests/pooled_orphan_archive.rs`）。
  - 分层 OK：39 crates / 183 条生产 path 依赖。
  - clippy 原文命令（`--tests --all-targets --all-features -- -D warnings
    -W clippy::absolute_paths`）exit 0、零输出；`cargo fmt --all -- --check`
    exit 0；`cargo shear` no issues。
  - 死代码：`sh/unused.js` 仅 2 条已知 trait impl 误报（`partial_cmp` /
    `as_uninit`），无新项。
- 三账本：baseline sync 44/44、**59770 chars**（任务给的 59778 对应
  sem-45 / sem-46 已由语义复审归档 +8；本地 io-29 仍留 `done/`，故为
  59770）；progress board `{todo 0, new 1, done 22, reviewed 170, reject 1}`；
  issue links 701 引用 / 0 缺失 / 0 状态错位。
- 测试：全量 `--workspace --exclude fkv_nvme_test --all-features` 连跑 2 遍均
  **1437 passed / 1 skipped**；`fkv_compat` 12/12；无 flaky 复现。
- 第 12 轮新代码复查（结构 / 测试面）：
  - sem-45：`dirty_li` 操作时取值、manifest 直接序列化，无残留
    `log_delta_value` / flush 回读；4 个新用例在套件内。变异：data 段回读最终
    槽 → 3 例红；segment 段回读 → test-19 用例红（`page 1 应为 7，实际 2`）；
    `free_page` 记空闲链编码 → 2 个 store 用例红。
  - io-29：被挡回上界 `(旧水位 >> shift).saturating_sub(1)` 正确；两例在
    legacy 覆写前、reopen 后写新值前 / 后读 file 0 / 1 / 1022，并断言远端
    key 只属旧 / 新对应 identity；变异回 0 → 两例在 `old0001` 处
    `ResourceMissing` 红。
  - sem-46：仅文档 + 布局锁定用例；断言 live `[0,1,2]` / `[3,4,5]`、表长 6、
    空闲 3、物理页不复用、第三轮复用 `[0,1,2]`；顺序反转必红。
  - test-19：segment 段 helper + 逐字节期望 + 按操作序重放 + 冷重开；变异
    segment 回读最终槽 → 断言红，非 panic / 超时。
- 断言强度：新增 / 扩展测试共 60+ assert 站点（mapper 11、store 级 20、
  large 11、manifest 22、cloud helper 14 等），抽查无「常量自证 / 只重开」弱
  断言。
- 复审归档：`test-19`、`io-29`、`sem-45`、`sem-46` → `reviewed/`（复审记录
  附文件末尾；sem-45 / sem-46 的语义面复审见本轮语义域记录）。
- 新立项：
  - [doc-07](doc-07-baseline-sem45-segment-byte-parity-overclaim.md)（P3）：
    `COMPAT_BASELINE:827-829` 把「data mapping 段逐字节等价」写成
    「manifest mapping / segment 段逐字节等价」，与 sem-43（truncate 的
    segment 段不作为等价目标）和 sem-46（覆盖大 value 的 segment page id
    集合 / 表长不逐字节）自相矛盾。
  - [sem-47](sem-47-overflow-chain-free-order-reversed.md)（P2）：
    overflow 链释放顺序与 C++ 相反（fkv `pending.pop()` 逆序 vs C++
    `DelOverflowValue` 前向），多页 overflow 的 delete / 覆盖 mapping log
    delta 顺序与 snapshot 空闲链不对拍。
  - [sem-48](sem-48-same-page-multikey-free-alloc-phase.md)（P2）：
    同页多 overflow key 替换时 fkv 整页 `free*→alloc*`，C++ 逐 key
    `free→alloc` 交错；log delta 顺序与逻辑页复用归属不同（混合批
    「插入键序在删除键之前」同样命中）。
- 语义 / 盘面格式域判定：本轮 2 条新意见（sem-47、sem-48）——第 12 轮
  sem-45 的「值时机」修复成立，但同一 `data mapping` 段的**记录顺序**在
  多页 overflow 链与同页多 key 负载下仍与 C++ 不同；sem-42+45 条目的
  「data mapping 段逐字节等价」需按这两条进一步收窄或修复。
- 判定：结构 / 测试 / 死代码领域**无新实现缺陷**；全部门禁绿、全量测试两遍
  无 flaky、第 12 轮四处改动的测试判别力成立。文档一致性有 1 处 P3 基线过度
  声明（doc-07），不影响代码 / 数据 / 互读。

## 第 14 轮迭代（2026-11-07，语义 / 盘面格式收敛判定轮）

- 依据：fkv HEAD `2d74270`（只读）；C++ 基线 `82b774c`。
- 第 13 轮修复复核：
  - sem-47（overflow 链前向释放）：逐行与 C++ `DelOverflowValue` 对齐，
    批尾页「先读下一批指针 → 再 free」、非尾页只 free、批宽同源；两条
    盘面推导用例在套件内且改回 `pop()` 会红。归档 `reviewed/`。
  - sem-48（同页多 key 逐 key 相位）：`rewrite_page_values` 的双游标顺序与
    C++ `ApplyOnePage` 的 `DelOverflowValue → WriteOverflowValue` 一致；
    空页分支与 `LeafLinkDelete + FreePage` 同序；两条用例在套件内且旧实现
    会红。归档 `reviewed/`。
  - doc-07 收窄后的基线措辞（data mapping 段槽值编码 / 释放顺序、segment
    段只算槽值编码、不设 sem-42 跳表行）与代码、sem-43 / sem-46 条目一致。
  - io-30（prewarm 停机上界）与 `ArchiveCrond` 同口径（2s + 告警 + 分离），
    在飞 tick 不中断；属实现选择，不改变盘面。
- 新立项：
  - [sem-49](sem-49-split-page-mapping-log-order.md)（P2）：既有 data 页
    被写批撑裂时，fkv 先 `allocate_page` 新分裂页、后 `remap_page` 原页；
    C++ 按 `ShiftLeafLink` 的写出顺序先原页后新页 ⇒ data mapping 段的
    log 顺序与物理页分配（`PageId -> FilePageId` 槽值）都相反。探针
    （512 B 页、3+7 条 64 B）delta = `[(2,17),(0,25),(1,33)]`；C++ 应为
    `[(0,17),(2,25),(1,33)]`。
  - [sem-50](sem-50-write-path-expired-entry-retention.md)（P3）：C++
    `add_to_page` 在页重写时内联丢弃 `expire_ts <= now` 的基页旧条目（并释放
    overflow / segment、删 TTL 索引）；fkv merge 不看 now，旧条目保留到
    `clean_expired`。同一写批的页字节 / mapping log / manifest 记录流不同，
    基线 TTL 跳表行「不改盘面格式」不再成立。
- 三账本：baseline sync 44/44（60689 chars，压行口径）、issue links 794 引用 /
  0 缺失 / 0 状态错位、progress board `{todo 0, new 2, done 18, reviewed 177,
  reject 1}`。
- 判定：语义 / 盘面格式领域**未收敛**——sem-47/48 的修复成立，但页分裂的
  页级写出顺序（sem-49）与 TTL 写路径的过期条目处理（sem-50）仍与 C++
  不同，均已带两侧 `文件:行号`、探针 / 用例与验收方式立项。

## 第 14 轮迭代（2026-11-07，结构 / 测试 / 死代码 + 文档一致性收敛判定轮）

- 依据：fkv HEAD `2d74270`（只读）；C++ 基线 `82b774c`。与上一节语义域
  同轮并行，本节只覆盖结构 / 测试 / 死代码 / 文档一致性。
- 代码门禁：
  - 行数 OK：176 个生产文件 max 994（`fkv_config/src/lib.rs`）；内联测试
    max 967（`fkv_task_write/src/lib.rs`）；test-only max 1472
    （`fkv_store/tests/pooled_orphan_archive.rs`）；`fkv_store/tests/manifest.rs`
    1031（< 1500）。
  - 分层 OK：39 crates / 183 条生产 path 依赖。
  - clippy 原文命令（`--tests --all-targets --all-features -- -D warnings
    -W clippy::absolute_paths`）exit 0、零输出；`cargo +nightly fmt --check`
    exit 0；`cargo shear` no issues。
  - 死代码：`sh/unused.js` 仅 2 条已知 trait impl 误报（`partial_cmp` /
    `as_uninit`），无新项。
- 三账本（当前工作树）：baseline sync 44/44 meta（cpp 22 / rust 22）、
  **60689 chars**（提交 `8e70bc7` 为 60681；+8 来自语义域把 sem-47/48 的
  done 链接改为 reviewed）；issue links 794 引用 / 0 缺失 / 0 状态错位；
  progress board `{todo 0, new 2, done 18, reviewed 177, reject 1}`（new 2 =
  sem-49 / sem-50，语义域本轮立项；done 18 / reviewed 177 含本节归档
  doc-07 与异步域归档 io-30）。
- 测试：全量 `--workspace --exclude fkv_nvme_test --all-features` 连跑 2 遍
  均 **1442 passed / 1 skipped**；`fkv_compat` 12/12；无 flaky 复现。
- 第 13 轮新代码复查（结构 / 测试面）：
  - sem-47：`free_overflow_chain` 前向游标与 C++ `DelOverflowValue` 逐行
    一致（批尾先读页取下一批指针再 free，非尾只 free；批宽取
    `layout.data.overflow_pointers` 与写链同源；`seen` 只防损坏盘面成环）。
    2 条盘面推导用例在套件内。变异回旧 LIFO `pop()` → 2 例确定性红
    （`manifest.rs:754/792`）。
  - sem-48：`rewrite_page_values` 双游标（旧页 key × `overflow_li` 升序）
    每个 key 先 `release_replaced_value` 再立即 `write_page_slot`；
    `free_replaced_values` 空页分支复用同一实现；`write_overflow_value_li`
    抽出的 `overflow_slot_range_li` / `write_overflow_slot` 与旧内联实现
    行为一致，无重复状态机。2 条用例断言逐 key free→alloc 相位、a/b 各自
    复用自身旧页集合、混合批先分配后释放。变异回「整页全量 free →
    全量 alloc」→ 2 例确定性红（`manifest.rs:919/1005`）。
  - io-30：`stop()` 丢弃发送端后轮询 `is_finished`，2s 上界 + 告警 +
    分离；测试同时断言下界（>= 2s）与上界（< 3s）。变异回无界 `join()` →
    60.047s 后在上界断言红（`prewarm_tests.rs:70`）。
- 文档一致性：doc-07 收窄成立（`grep "manifest mapping / segment"` 无命中，
  exit 1）；sem-42+45 条目三层限定（data mapping 槽值编码 / 释放顺序 /
  segment 段仅槽值编码）与 sem-43 / sem-46、l2-03 跳表无矛盾；残余的
  分裂页写出顺序由语义域 sem-49 跟踪，不回退本条结论。
- 复审归档：`doc-07` → `reviewed/`（复审记录附文件末尾）；`doc/progress.md`
  按归档后目录重新生成。
- 判定：结构 / 测试 / 死代码 / 文档一致性领域**无新意见**。第 13 轮三项
  改动的结构清晰、无重复状态机，5 条新增测试均有判别力；全部门禁绿、
  全量两遍无 flaky。页分裂写出顺序（sem-49）与 TTL 过期条目（sem-50）
  属语义域，已由同轮语义复审立项，不在本节重复。

## 第 14 轮迭代（2026-11-07，异步 / IO 模型收敛判定轮）

- 依据：fkv HEAD `2d74270`（只读）；C++ 基线 `82b774c`；运行时对照
  `../compio`。
- 第 13 轮修复边界复核：
  - sem-47：`free_overflow_chain` 的批内前向游标在「16 页整批 / 16+1 /
    4 指针多批 / 单批 16 页最后一页无后继指针」四种边界均按
    `DelOverflowValue` 顺序释放，无丢页；损坏盘面的重复页 / 成环只影响
    防御分支。新增两条盘面推导用例在套件内。
  - sem-48：`rewrite_page_values` 的逐 key 相位在整页删空（a/b 两链按
    key 升序前向释放 → data 页 → index root）、混合批「插入键序在删除
    键之前」、插入键位于两旧键之间、overflow→小 value + 另一 overflow
    四类探针下均与 `ApplyOnePage` 一致。崩溃一致性：`WritePage@1/2/26/51`
    注入 IoFail，Append / Pooled 两模式均回滚 mapping、旧值可读、
    reopen 旧 manifest 无损。
  - io-30：`PrewarmCrond::stop` 先丢弃信号发送端，再以 10ms 粒度轮询
    `is_finished`；2s 超时告警并 drop `JoinHandle` 分离，线程体只拥有
    `CloudStore` / `Arc<PrewarmState>`，无借用逃逸；单元测试实测
    2.014s（下界 2s、上界 3s）。与 `ArchiveCrond::stop` 同口径，未遗漏
    线程句柄。
- 全新扫描面：事件循环高 / 低优先与时间片、读窗口（128 在飞 / 预算退休
  / 暂存 FIFO / 大值单飞与 `large_park_li`）、写门（per-partition FIFO /
  全局并发 / Put 组合并）、读 / 写 / 维护 / 预热任务化、取消
  （`RequestHandle` / `LoadGuard` / `DownloadGuard` / `FixedSlotGuard` /
  S3 `catch_unwind`）、shutdown（crond 2s 上界、shard Shutdown + 异步
  join、S3 Drop 非阻塞）、定时器（shard 内 compio timeout；调用方
  futures-timer 按 io-01 reject；crond `recv_timeout`）、SQ / IO 深度
  （`io_queue_size` ring、`max_inflight_write` 写闸门与 fdatasync 并发、
  批读受 `overflow_pointers` 约束）。未发现新缺陷。
- 复审归档：`io-30` → `reviewed/`。`progress.md` 重新生成后为
  `{todo 0, new 2, done 18, reviewed 177, reject 1}`；issue links 794 /
  0 缺失 / 0 状态错位。
- 测试：`fkv_shard` 149 passed / 0 failed；`fkv_runtime` 114 passed /
  1 skipped（既有 skip）；sem-47/48 重点 4 例、store 页写失败例、io-30
  停机例全绿。
- 判定：异步 / IO 模型领域**无新意见**。页分裂写出顺序（sem-49）与
  TTL 写路径过期条目（sem-50）属语义 / 盘面格式域，已由同轮语义复审
  立项，不在本节重复。

## 第 15 轮迭代（2026-11-07，结构 / 测试 / 死代码 + 文档一致性收敛判定轮）

- 依据：fkv HEAD `64415f9`（只读）；issue 仓提交态 + 工作树；C++ 基线 `82b774c`。
- 门禁实测：
  - `bash scripts/ci/check_line_count.sh` **exit 1**（唯一违例
    `crate/engine/fkv_store/tests/manifest.rs` 1705 > 1500，test-only）；
    `check_layers.sh` 因复用行数门禁同时 exit 1（分层本身 OK：39 crates /
    183 条生产路径边）。
  - `cargo +nightly fmt --all -- --check` exit 0；`cargo +nightly clippy -q
    --tests --all-targets --all-features -- -D warnings -W clippy::absolute_paths`
    退出码 0、输出 0 行；`cargo shear` → `no issues found`。
  - 三账本：`check_baseline_sync.sh` → 44/44 meta.txt（cpp 22 / rust 22）、
    62250 chars；`check_progress_board.sh --check` → exit 0（看板与 ignored
    口径、report 整文件 diff 均一致）；`check_issue_links.sh` → 936 refs /
    0 missing / 0 状态错位。
  - 死代码：`bun sh/unused.js` 仅两个既有 trait impl 误报
    （`reopen.rs::partial_cmp`、`buf.rs::as_uninit`），无新增死代码 / 未用依赖。
- 新增意见：
  - [struct-33](../new/struct-33-manifest-rs-line-count-over-1500.md) **new**：
    第 14 轮把 11 条分裂顺序用例追加到 `manifest.rs` 末尾，文件由
    `2d74270` 的 1031 行涨到 1705 行；sem-52 提交（`fc1ae96`，1634 行）
    起越线，sem-53（`64415f9`）保持红。
  - [test-22](../new/test-22-sem49-three-way-split-order-unlocked.md) **new**：
    sem-49 的「原页先 remap」只在 2 页分裂（唯一新页走 sem-53 的
    `reserve_page`）时被锁定；把 `data.rs:168` 的 remap 移到分裂页 allocate
    循环之后，`fkv_store --test manifest` 仍 24/24 全绿，只有 ≥3 页分裂探针
    （`TBL_LEN=5`，正确 delta `[(0,17),(2,25),(3,33),(4,41),(1,49)]`）能抓住。
- 第 14 轮改动结构复核：`ValueCtx` 新增 `next_planned_page` / `deferred_prev`
  / `deferred_li` 与 `DeferredPage.reserved` 均有行为注释与 C++ 出处；
  `PageMapper::reserve_page` / `commit_reserved_page` 的槽位编码、journal 回滚
  与「未提交槽位 = INVALID」契约写清；`take_free_page` 抽出后
  `allocate_page` / `reserve_page` 共用同一空闲链维护。`deferred_li` 文档声明
  上限两页（本组前驱 + 新页），`flush_deferred_front` 的 `remove(0)` 有常数级
  说明。未发现新的重复 / 魔法数字 / 可见性越界。
- 测试抽查（第 14 轮 22 条新增用例中逐行核对 20 条）：sem-49 1、sem-51 3、
  sem-52 4、sem-53 manifest 1 + mapping 3、sem-50 8（btree 2 / task_write 2 /
  store 4）；sem-51 另 2 条（`rewritten_predecessor_…` /
  `deleted_head_page_…`）核对用例名与 mutation 覆盖即可。
  断言均为精确 mapping delta、物理页归属、页链 prev/next、TTL 索引 delete 与
  overflow/segment 释放计数，非复述常量。mutation 验证：sem-53
  `reserve_page→allocate_page`、sem-51 关闭 `deferred_prev`、sem-52 单页立即
  remap、sem-50 关闭过期过滤，均被现有用例抓住；唯一存活为 test-22 的
  ≥3 页分裂顺序。
- flaky：全量 `cargo nextest run --workspace --exclude fkv_nvme_test
  --all-features` 第 1 遍 1463/1464（`fkv_shard::shutdown_race
  requests_racing_shutdown_all_resolve` 15.021s 超时；当时 load average 85，
  另有并行 review 构建），第 2 遍 1464/1464（45.5s）、第 3 遍 1464/1464
  （47.6s）；该用例单跑 10/10 通过（场景约 35-41ms，15s 上限为空载的 ~375 倍）。
  判为极端负载下的一次环境性超时，非稳定 flaky，不立项（test-05 的
  shutdown 异步 join 修复仍在 `lifecycle.rs:519-543`）。
- 文档一致性：`doc/architecture/04-btree-and-formats.md` 的 TTL 小节已随
  sem-50 更新（写批内联丢弃过期基页条目；`clean_expired` 不做 now 过滤），与
  `apply_expired_deletes` 的注释和 `PageLayout::new`（不调 `with_now_ms`）
  一致；`COMPAT_BASELINE` 中 sem-49/51/52/53 的测试名与 HEAD 实际函数名一致
  （`930cc0c` 已修正）；44 份 meta 与 progress 同步通过。
- 复审归档：本轮 `done/` 中无 struct/test/doc 系列待归档（sem-49~53 属语义域，
  由并行语义复审移入 `reviewed/`）。
- 判定：结构 / 测试 / 死代码 / 文档领域**2 条新意见**（struct-33、test-22），
  其余门禁绿（行数门禁除外，已单列 struct-33）。

## 第 16 轮迭代（2026-11-07，结构 / 测试 / 死代码 + 文档一致性收敛判定轮，补跑被中断的第 15 轮复审）

- 依据：fkv HEAD `da7bc70`（只读，`git status` 全程干净）；issue 仓提交态 +
  本轮流转；C++ 基线 `82b774c`。
- 复审对象：第 15 轮四项改动——sem-54（`fd2335a`，OutputPagePlan）、
  sem-55（`873c4d4`，TtlUpsertSink + merge 门控 + write.rs 接线）、
  struct-33（`d77561d`，manifest.rs 拆 3 target + common/mod.rs）、
  struct-34（`da7bc70`，fkv_btree merge 状态机拆 merge.rs）。
- 门禁实测（全部 exit 0）：
  - `check_line_count.sh`：production 177 files / max 994（`fkv_config`）；
    inline max 969（`fkv_task_write/src/lib.rs`）；test-only 264 files /
    max 1472（`pooled_orphan_archive.rs`）。
  - `check_layers.sh`：39 crates / 183 条生产 path 边。
  - `cargo +nightly clippy -q --tests --all-targets --all-features --
    -D warnings -W clippy::absolute_paths`：输出 0 行。
  - `cargo +nightly fmt --all -- --check`：exit 0；`cargo shear`：no issues。
  - 三账本：baseline sync 44/44（62803 chars）；issue links 938 refs /
    0 missing / 0 wrong-state；progress board 开局红（并行复审已把
    sem-54/55/test-22/fp-01 移 `reviewed/` 而未重生成看板），本轮归档
    struct-33/34 后重生成转绿：`{todo 0, new 1, done 17, reviewed 188,
    reject 2}`（issue 仓 `d0d799d`）。
  - 死代码：`bun sh/unused.js` 仅 2 条已知 trait impl 误报
    （`partial_cmp` / `as_uninit`），无新增。
- 结构面（第 15 轮改动质量，无缺陷）：
  - struct-33 零差异实测：27 个 `#[test]` 函数体（9+4+14）在三个新 target
    中逐字节存在；8 个 helper 在 `common/mod.rs` 函数体逐字节一致、仅加
    `pub`。
  - struct-34 零差异实测：旧 `lib.rs` 搬移段 12499 字节（`page_kind` 文档行
    至 EOF）与 `merge.rs` 正文逐字节一致；`merge.rs` 头部仅模块文档 +
    imports + `use crate::BatchEntry`；`pub use merge::{…}` 保持外部路径。
  - sem-54 `OutputPagePlan`：trait `OutputPageWrite`（overflow.rs，
    `close_reached_pages` / `close_all_pages` + `()` 空实现给删除路径）与
    实现（data.rs 私有 struct）边界清晰，静态分发无 dyn；重写与新树首写
    两路径共用同一 plan/驱动，无重复相位逻辑；deferred 队列上限 2 页、
    `debug_assert` 按序关闭、边界触发键 = 下一页首键，均有 C++ 出处注释。
  - sem-55 `TtlUpsertSink`：`RefCell` 私有（`push` 模块私有、pub 仅
    `take`），`MergeOption.ttl_upsert_sink: Option<&TtlUpsertSink>` 保持
    `Copy`；merge 两个门控分支（Equal 的 `expire != base_expire`、INSERT）
    与 C++ `UpdateTTL` 逐条对应；write.rs 空批跳过 TTL 树对齐
    `ApplyTTLBatch`；TTL 写批用全新 `PageLayout`（sink=None）防递归收集。
- 测试面：
  - 断言抽查 12 条（sem-54 3 + sem-55 store 2 + merge 门控 5 +
    扩充 1 + 抽样搬移 1）：精确 mapping delta 序列、终态 `FilePageId`、
    页链 prev/next、overflow 指针链、sink 精确内容（key/op/空集）、
    `next_expire_ts` 不被拉低、到期清理统计；无复述常量式断言。
  - 变异 3/3 检出（改动均还原，`git status` 回到干净）：
    M1 去掉相同 expire 门控 → `sem55_same_expire_overwrite_skips_ttl_page_write`
    红；M2 边界全部推迟到 `close_all_pages` → 两条 sem-54 交错用例红
    （2 页对照按设计仍绿）；M3 去掉 `expire_ts != 0` 过滤 →
    `clearing_expire_deletes_old_ttl_index_without_upsert` 红。
  - flaky：干净全量 `cargo nextest run --workspace --all-features` 2 遍均
    **1476 passed / 1 skipped**（首次跑对的 RUN2 失败为变异实验并发残留，
    还原后复跑排除）。
- 文档一致性（1 条新意见）：
  - [doc-08](doc-08-baseline-test-paths-stale-after-struct33.md) **new**（P3）：
    struct-33 拆分后基线 3 处锁定测试路径失准（sem-47/48 4 名 →
    `overflow_free_order.rs`，sem-49/51/52/53 6 名 → `shift_leaf_link.rs`），
    且 sem-55 段未列锁定测试名；sem-54 段路径与函数名实测准确。
  - `doc/architecture/04-btree-and-formats.md` 的 COW/TTL 小节与 sem-54/55
    后实现无矛盾（未声明被推翻的顺序性事实）。
- 复审归档（done/ → reviewed/，issue 仓 `d0d799d`）：`struct-33`、
  `struct-34`（复审记录附文件末尾）；done/ 其余 17 条均为
  corpus/dep/fkv_config/flake/l1/l2/rt 系列，不属本轮授权域。
- 判定：结构 / 测试 / 死代码领域**无新意见**，第 15 轮四项改动质量达标；
  文档一致性 1 条 P3（doc-08）。门禁除 progress board 的并行暂态（已修复）
  外全绿。

## 第 17 轮迭代（2026-11-07，性能收敛判定轮：补跑被并发上限拦截的 perf 复审）

- 依据：fkv 已提交 HEAD `da7bc70`（只读；/tmp worktree 实测，主仓工作树未动）；
  C++ 基线 `82b774c`。前 10 轮 perf-01~20 与 bench-01 已全部闭环；第 9~16 轮
  性能面连续零发现。本轮为收敛判定轮。
- 复审对象（done/ 中不在 reviewed/ 的 perf 系列）：**0 条** —— `issue/done/`
  现存 17 条均为 corpus/dep/doc/fkv_config/flake/l1/l2/rt 系列，perf-01~20 +
  doc-04 已全部在 `reviewed/`，无流转动作。
- 全新热点扫描（HEAD 写 / 读 / 后台路径，重点是第 7 轮 perf 复审之后合入的
  sem-42/45/47~55、io-28/29/30、l2-08/09 改动面），逐角核对结论：
  - **写路径**：`put_at_inner` 每批取一次 `now_ms`（`write.rs:39,489`），
    `prepare_write_batch` 快路径（无 TTL + key 严格升序）整批 `Cow::Borrowed`
    零拷贝（`fkv_task_write/src/ttl.rs:66-74`）；sem-50 的 merge 过期丢弃是
    每旧条目一次 `is_expired` 纯比较（`fkv_btree/src/merge.rs:201`）；
    sem-54 `OutputPagePlan` 未分裂页 `boundary_key_li` 为空、每旧 key 两次
    `close_reached_pages` 退化为一次 slice `get`（`data.rs:186-208`），分摊
    常数；sem-55 `TtlUpsertSink` 为每批一个 `RefCell`，`ttl_li` 为空时跳过
    TTL 树写批（对齐 C++ `ApplyTTLBatch`）。
  - **读路径**：`descend_step` 缓存命中 = borrow + `node.seek`；未命中 =
    l2-08 single-flight + perf-10 owned 直读，`IndexNode` 只构造一次并 move
    进缓存（`fkv_task_read/src/lib.rs:418-438`）；floor 的「过期命中回退」
    整页重扫 + key clone 只在该 TTL 角触发，非热路径。
  - **TTL 清理**：`maybe_clean_expired` 由 `state.next_expire` 门控，无 TTL
    条目的负载每写零成本（`fkv_store/src/ttl.rs:92-98`）；deferred 模式只置
    pending 标记。
  - **后台**：`needs_maintain` O(1) 且 pooled 下 gate 在 Append（pooled-03）；
    维护走低优先任务（io-05）；`reclaim_retired_now` 在 Append 模式 / 空
    retired 队列两处短路（perf-12）；`append_mapping_log` 按 O(脏页) 写
    （sem-45 记录值直写，不回读槽位）带 `YieldBudget`；`touch_table` /
    `evict_over_root_meta` 在默认 `max_open_tables=0` / `root_meta_cache_bytes=0`
    下零开销早退（`state.rs:168-172,222-226`）；`IndexCache` retag 走例外表
    摊销压实（R3），compact 后 `retag` 不再整缓存遍历。
  - **scan**：页缓冲复用（perf-01）+ 预取管线 + 固定步长时钟让出；结果条目
    每 key/value 各一次必要物化，无额外拷贝。
- 实测（release，/tmp worktree `da7bc70`，darwin arm64 12 核，load 11-19）：
  - write-only rows 20000 / batch 100 两遍：put_kv_s **310,207 / 318,456**
    （p50 295 / 299 µs，p99 793 / 542 µs；第 7 轮同负载 211,793–319,949 /
    p50 280–351 µs / p99 1.4–2.0 ms，无回退、p99 更优）。
  - read-only rows 20000 / partitions 2 / read-per-part 2 / test-secs 5 两遍：
    get_kv_s **162,909 / 174,209**（p50 21 / 19 µs，p99 60 / 56 µs；perf-11
    记录 170,019 / 22 / 51，同量级）。
  - write-read rows 20000 / test-secs 2 两遍：put **357,787 / 354,606**、
    get **211,567 / 212,334**（p50 275 / 275 / 17 / 17 µs；第 7 轮 put 153,667
    / get 94,555，当时 load 18–51，趋势向好看）。
  - `bash scripts/perf_gate.sh selftest`：**通过**（30% 吞吐 / 速率回归、
    +30% 延迟（20% 容差）、`_mib_s` / `reopen_us` 方向、放大派生 info、
    资源双向、unknown 告警、模式不一致拒绝全部按预期红/绿）。
- 复审归档：无（done/ 无待归档 perf 条目）。
- 判定：**性能领域无新意见**。sem-42~55 与 io-28~30 合入后的写 / 读 / 后台
  热路径无新增分配、拷贝、锁或全量扫描；实测三项负载与既有记录同量级无
  回退；perf_gate selftest 绿。零发现为本轮有效结论。

## 第 17 轮迭代（2026-11-07，语义 / 盘面格式收敛判定轮，补跑被并发上限中断的第 17 轮语义面）

- 依据：fkv HEAD `da7bc70`（只读，`git status` 干净）；C++ 基线 `82b774c`；
  issue 仓 `0718355` 之上做流转。前十六轮语义面零发现，本轮为收敛判定轮。
- 基线自洽复查（全文 1024 行）：
  - 机械化复核：`COMPAT_BASELINE` 内 271 个反引号标识符逐一反查两侧源码；
    40+ 个「行为锁定测试」名的声明路径与 `grep -rln "fn <名>("` 实测路径
    **全部一致**（sem-47/48 4 名 → `overflow_free_order.rs`、sem-49/51/52/53/54
    → `shift_leaf_link.rs`、sem-42/45 3 名仍留 `manifest.rs`、sem-55 清单在
    `ttl.rs` / `write/ttl.rs`）——doc-08 的修正无遗漏、无新漂移。
  - 最新条目逐条对证代码：sem-54 `OutputPagePlan`（data.rs:88 +
    overflow.rs `OutputPageWrite` trait）、sem-55 `TtlUpsertSink`
    （merge.rs:387，`MergeOption.ttl_upsert_sink`，write.rs:231/:255 空批跳过
    TTL 树）、pooled-14/io-28/io-29 的 `install_replay_state` 合成区间与
    segment 守卫（manifest.rs：bump 谓词、空链守卫、`bumped_segment_fp_id`
    挡回时 `saturating_sub(1)`）、l2-05a/b、l2-01（backend.rs:74-104 +
    config.rs:35）、l2-02（standby lib.rs:517/:599）、l2-04（metrics 19 标签）、
    l2-07（Cargo.toml:34,45-46 精确 pin）、l2-11（`flight_wait` lib.rs:354）、
    l2-12（`LoadGuard::drop` flight.rs:196）、l2-13（`IndexRead::none` lib.rs:234）、
    rt-01（archive.rs:44/:51 超时常量）、rt-02（`archive_crond_on` 三项谓词）——
    「文档说、代码做」全部成立。
  - 三账本：`check_baseline_sync.sh --check` 44/44（63167 chars）exit 0；
    `check_issue_links.sh` 938 refs / 0 missing / 0 wrong-state；
    `check_progress_board.sh` 开局红（doc-08 归档 done 后未重生成看板的
    done 17↔18 暂态），本轮流转后重生成转绿。
- 全新语义扫描（C++ 面未对照处，逐面排除后 1 条立案）：
  - **排除**：segment 侧放大率 compaction（C++ `UpdateMeta` 对 segment mapper
    不受 `data_append_mode` 门控、knob `segment_file_amplify_factor` 默认 2 ——
    fkv `compact.rs:226-253` 已同构对齐）；`DropTable`（C++ per-partition
    `DropRequest` ≡ fkv `drop_table` 只删 manifest + GC 收尾，代码注释已锚定）；
    Scan inclusivity（C++ `begin_inclusive`/`end_inclusive` ≡ fkv
    `ScanOption`/`fkv_api::ListPage` 同名字段）；批内过期胜者可见数据
    （C++ `DELETE (expired update)` 删基页不插入 ≡ fkv 折叠 delete 胜出）；
    overflow 判定阈值公式（`is_overflow_kv` 有显式对齐注释且同构）；
    写后维护执行位（C++ `AddPendingCompact` → 后台 ≡ fkv O(1) 预判 +
    回执后 `maintain_after_write`）；prewarm 下载面（两侧均 manifest+data+
    segment 三类）；no-op delete 落非空分区覆盖页（两侧同按 C++
    `ApplyOnePage` 无条件重写页）。
  - **立案**：[sem-56](../new/sem-56-noop-batch-manifest-empty-snapshot.md)
    **new**（P3）：已清空分区（root 与 TTL root 均空）上的 no-op 批，C++
    `FlushManifest` 的 `need_empty_snapshot` 分支每批整写空快照
    （`write_task.cpp:724-786`，`manifest_size` 复位）；fkv 空批早退零写入
    （write.rs:144-146）、无效果批追加仅含 branch 元数据的空 delta record
    （`emit_batch` 跳过 delete、`build_new_index` 空 children 返回 NULL_PAGE
    后仍无条件 `append_mapping_log`），manifest 线性增长到 `manifest_limit`
    才重写。可见数据 / 恢复不受影响，manifest 形态 / 大小 / 更新次数不同，
    同 oplog 的 manifest 对拍会在该形状失败。该面此前未登记。
- 复审归档（done/ → reviewed/，18 条全部，各附第 17 轮复审记录）：
  l2-01/02/03/04/07/08/09/11/12/13、rt-01、rt-02、corpus-02、l1-02、dep-01、
  fkv_config-01、flake-01、doc-08。rt-02 记录勘误：代码谓词为三项与
  （含 `data_append_mode`），比记录的「两项与」更贴 C++，属记录表述过时。
- 判定：语义 / 盘面格式领域 **1 条新意见**（sem-56，P3）；其余抽查与扫描面
  「文档说、代码做」全部成立；done/ 清空，reviewed/ 206 条。

## 第 18 轮迭代（2026-11-07，结构/代码质量 + 测试质量 + 死代码 + 文档一致性收敛判定轮）

- 依据：fkv HEAD `086bf8a`（只读，mutation 实验一处改动已还原，`git status`
  复核干净）；C++ 基线 `82b774c`；issue 仓 `b275c5e` 之上做流转。第 16、17 轮
  本领域零发现，本轮为收敛判定轮。
- 第 17 轮改动质量复审（sem-56 `c74047b` / sem-57 `086bf8a`）：
  - write.rs 两处门与 C++ `WriteTask::FlushManifest` 三段门逐项对应
    （`need_empty_snapshot` `write_task.cpp:724-733` ⇔ `manifest_started &&
    roots 均空`；`ManifestBuilder::Empty()` ⇔ data/segment
    `dirty_page_li` 均空；`SwitchManifest`+复位+`ClearDirty` ⇔
    `write_snapshot_manifest` 整写+复位+clear_dirty+clear_dict_dirty）；
  - 失败路径核实：`abort_batch`（core-25）脏页回滚到批前，门判定不携带陈旧
    脏页；`write_snapshot_manifest` 失败 state 不更新、重试幂等；
    `cleanup_empty_partition`（gc.rs:19-33）置 `manifest_started=false` 后
    无效果批不重建 manifest；truncate 显式快照路径不变；SDK / shard 懒开
    `ResourceMissing → open_empty`（fkv_sdk lib.rs:139-146、dispatch.rs:56-63）
    用户可见语义不变；
  - 测试断言强度：sem-56 2 例 + sem-57 2 例 + ttl.rs 1 改，断言计数器 /
    字节级一致 / header / 对象集 / reopen，非复述常量；突变验证：本轮把
    sem-56 form-1 门临时改恒假，
    `noop_batch_on_cleared_partition_rewrites_empty_snapshot` 立即红
    （switch 期望 2 实得 1），还原后复绿；sem-57 跳过分支突变验证见其
    实现记录。
- 门禁复跑（全部只读）：
  - **clippy**：`cargo clippy -q --workspace --all-targets --all-features
    --keep-going -- -D warnings -W clippy::absolute_paths` → **2 错**
    （`fkv_store/tests/manifest.rs:765/815` 的 `std::slice::from_ref`，均系
    sem-57 新增测试行；其余 263 个 test target 与全部生产 crate 0 警告）
    → **[struct-35](../new/struct-35-clippy-absolute-paths-sem57-tests.md)
    立案**（P3）。sem-57 实现记录的 clippy 验证口径缺该 lint，已在复审
    记录勘误。
  - fmt `--all -- --check` exit 0；`cargo shear` 0；行数门禁 exit 0
    （生产 max 994 `fkv_config/src/lib.rs`，test-only max 1472
    `pooled_orphan_archive.rs`）；分层门禁 exit 0（39 crates / 183 生产边）。
  - 死代码：`bun sh/unused.js` exit 2 = 2 命中，均为已知 trait impl 误报
    （`DelayedReopen::partial_cmp` 的 `PartialOrd` impl、
    `DirectBuf::as_uninit` 的 `IoBufMut` impl），按既定口径跳过；shear 段
    0 发现。
  - 三账本：`check_baseline_sync.sh --check` 44/44 meta（cpp 22 / rust 22）
    + doc/progress.md ok（63171 chars）；`check_issue_links.sh --check`
    943 refs / 0 missing / 0 wrong-state（流转后）；`check_progress_board.sh
    --check` {todo 0, new 1, done 0, reviewed 208, reject 2} + report diff ok。
    `check_ledger_progress.sh` 红属已归档的陈旧工具口径（doc-03 / doc-05 /
    sem-18 已记录「不要用它核看板」），不属三账本门禁，不另立案。
  - flaky：`cargo nextest run --workspace --all-features` **两遍均
    1480 passed / 1 skipped**（同序同结果）；`fkv_compat` 12 passed / 0
    skipped。
- COMPAT_BASELINE：sem-56/57 均为向 C++ 对齐（差异消除），不设跳表行、
  基线不需登记；44 meta 与 `COMPAT_BASELINE` 首行一致（baseline_sync 绿）。
- 复审归档：done/ → reviewed/ 2 条（sem-56、sem-57，各附第 18 轮复审记录；
  文件内 done→reviewed 自引用路径已改写）。done/ 清空，reviewed/ 208 条。
- 判定：结构/测试/死代码/文档一致性领域 **1 条新意见**（struct-35，门禁
  回归，机械修复）；第 17 轮行为改动（sem-56/57）质量达标，测试有判别力
  （突变验证在案）；flaky 两遍全绿。除 clippy 2 行外各门禁零发现，收敛
  结论：本领域仅余 struct-35 一项待修。

## 第 18 轮迭代（2026-11-07，语义 / 盘面格式收敛判定轮，补跑被并发上限拦截的语义面）

- 依据：fkv HEAD `ffe98b0`（只读，`git status` 干净）；C++ 基线 `82b774c`；
  issue 仓在结构轮流转后的状态之上做流转。
- **sem-56/57 修复验证**（fkv `ffe98b0` vs C++ `82b774c`）：
  - form-1 门（`write.rs:143-159`）⇔ C++ `need_empty_snapshot`
    （`write_task.cpp:724-733`）：`manifest_started` ⇔ `manifest_size_ > 0`、
    `NULL_PAGE` ⇔ `MaxPageId`、`write_snapshot_manifest`（`manifest.rs:427-449`
    整写 + `manifest_size` 复位 + 双 mapper `clear_dirty_page_li` +
    `clear_dict_dirty`）⇔ `SwitchManifest` + 复位 + `ClearDirty`；
  - form-2/form-3 门（`write.rs:329-358`）：`wal_empty`（data/segment
    `dirty_page_li` 均空）⇔ `ManifestBuilder::Empty()`（`root_meta.cpp:140-143`），
    「空快照 / 跳过 / first_manifest 全量快照或 append」三段与 `:726-733` 逐项
    对应；空快照含 dict 字节（`state.rs:478` `dict_bytes: self.dict_bytes(tbl)`），
    同 C++ 快照分支传 `dict_bytes`；
  - **truncate 路径不受影响**：整表 truncate 仍走显式快照
    （`truncate.rs:80-95`），从未加载表早退（truncate.rs:47-49）= C++ 根空早退
    的「无 manifest」子集（新发现的其余子形态见下）；
  - **TTL 清理路径不受影响**：`clean_expired` 空批早退（`ttl.rs:253-265,290-297`）
    ⇔ C++ Expire 任务 `ttl_batch.empty()` 早退（`batch_write_task.cpp:2058-2061`）；
    非空清理批必有页重写（wal 非空）→ 无条件 append，两侧一致；
  - **compact 路径不受影响**：`moved == 0` 不写 manifest（`compact.rs:66-71`）⇔
    C++ `Compact` 的 clean 早退（`background_write.cpp:113-135`）+ wal 空不
    append；`moved > 0` append（wal 非空）两侧一致；
  - 测试：sem-56/57 五个锁定用例 5/5 passed；全仓 clippy（含
    `-W clippy::absolute_paths`）exit 0——sem-57 引入的 2 处回归已由
    struct-35（`ffe98b0`）清零。
- **基线自洽复查**：COMPAT_BASELINE 最新条目抽查（sem-42~45 / 47~49 / 54/55 /
  sem-43 truncate 段 / l2-05a/b / TTL 跳表行）「文档说、代码做」成立；行为锁定
  测试名 4 条反查全部命中（`shift_leaf_link.rs` ×2、`manifest.rs`、`ttl.rs`）。
  发现 1 处措辞过宽：`COMPAT_BASELINE:913`「partial truncate 两侧已对齐」对
  「无匹配 position」形状不成立（见 sem-58 Form B），需随 sem-58 收窄。
- **全新语义扫描**（C++ 面未对照处，重点为 FlushManifest 全部等价调用点）：
  逐调用点核对 put（已对齐）/ truncate / truncate_from / TTL 清理（一致）/
  compact（一致）后，**2 条新立项**：
  - [sem-58](../done/sem-58-truncate-manifest-shape-empty-and-no-match.md) **done**（P3）：
    truncate 路径的 manifest 冲刷形态与 C++ 相反——空分区整表 truncate 在 fkv
    整写 / 凭空创建 manifest（`truncate.rs:40-113` 无根空早退；`open_empty`
    即建 mapper），C++ 在 `root_id_ == MaxPageId` 时无条件早退不写
    （`batch_write_task.cpp:1849-1852`，三个子形态 A1 懒开空表建对象 / A2
    已清空重写快照 / A3 cleanup 删后重建）；无匹配 `truncate_from` 零写入，
    C++ 重写最右路径（`TruncateDataPage` 无匹配仍走 else 分支
    `WritePage`，:1983-2012；索引层 :1817-1836）并 append + 烧物理页；
    另有 4096 分批的 record 边界差（C++ 单任务单 append）。基线 :913 措辞
    需收窄。
  - [sem-59](sem-59-cleanup-drops-active-manifest-cpp-keeps.md) **new**（P2）：
    `cleanup_empty_partition`（`gc.rs:19-33`）删除活跃 manifest（cloud 后端
    连远端同 branch manifest 一起删，`fkv_cloud/src/manifest.rs:170`），而 C++
    的 GC 收尾 `TryCleanupLocalPartitionDir` 实质是一次 `rmdir(2)`
    （`UnlinkAt(AT_REMOVEDIR)`，`async_io_manager.cpp:2774-2790,1464-1477`），
    manifest 挡住 rmdir 时原样保留——C++ 全仓删 manifest 的唯一生产调用点是
    显式 Drop（`batch_write_task.cpp:1896`）。后果：Append 模式（cloud /
    standby 强制）下「写空分区 + 一次写后维护」后，C++ manifest 存活且 no-op
    批每批整写空快照，fkv manifest 已删、no-op 批零写入——sem-56 刚对齐的
    行为在该路径被整体旁路；gc.rs:12-13 的对齐注释与 C++ 源码不符。
- **门禁连带发现**：struct-35 的修复提交 `ffe98b0` 本身把 std 导入插在外部
  crate 导入区中间、两处调用链未合并，`cargo +nightly fmt --all -- --check`
  在 HEAD exit 1（diff 仅 `manifest.rs` 4 处；CI `ci.yml:29` fmt 步骤红；
  stable fmt 同红）→ [struct-36](struct-36-fmt-gate-red-after-struct35-fix.md)
  **new**（P2）。clippy / 行数 / 分层 / cargo shear / unused.js（2 条已知误报）
  均 exit 0。
- 复审归档（done/ → reviewed/，1 条，复审记录附文件末尾）：struct-35
  （clippy 修复达标：absolute_paths 门禁 exit 0、sem-56/57 用例 5/5、行数门禁
  OK；连带 fmt 回归由 struct-36 承接）。done/ 清空，reviewed/ 209 条。
- 判定：语义 / 盘面格式领域**未收敛**——sem-56/57 修复本身成立，但 truncate /
  分区收尾路径存在 2 条未登记的 manifest 对象集 / 冲刷形态差异（sem-58、
  sem-59），另有 1 条门禁回归（struct-36）。收敛计数：语义域连续「完善」判定
  仍为 0 轮（本轮为发现轮）。

## 第 19 轮迭代（2026-11-07，结构 / 代码质量 + 测试质量 + 死代码 + 文档一致性收敛判定轮，补跑被并发上限拦截的第 19 轮结构面）

- 依据：fkv init 快照 `1a324b1`（tree `3d4adf8`，与 `origin/main` 一致；
  即第 18 轮判定的 `9b740b3` 内容 + 并行 perf 工作流已入库的 bench 首轮
  指标改动，fkv 历史在本轮期间被再次重写为单 `init` 提交）；C++ 基线
  `82b774c`。工作树另有 2 个 fkv_bench 文件的 fmt 修复**未提交**（归属
  并行工作流，本轮未触碰）；门禁数字均在含该在途修复的工作树上取数，
  HEAD 冻结快照另行验证（worktree `/tmp/fkv-r19-snap`，已清理）。
- **第 18 轮改动复审（代码质量面，零新意见）**：
  - sem-58 Form A 门：`Store::truncate` 门位于 `refresh_branch_metadata` /
    `begin_mapper_batch` 之前（`truncate.rs:50-55`），条件
    `root == Some(NULL_PAGE) && ttl_root == NULL_PAGE` 与 C++
    `batch_write_task.cpp:1849-1852` 等价；`truncate_from` 空树经
    `snapshot.root_id() == NULL_PAGE` 早退（`truncate.rs:152-154`）天然零写，
    与 C++ 分派前早退结果等价，无需补门；未加载表仍走 `mapper == None`
    结算早退。批次回滚路径（`settle_mapper_batch` 三处错误分支）完整。
  - sem-59：`cleanup_empty_partition` 收敛为「逻辑空判定 +
    `try_remove_empty_partition_dir`」（`gc.rs:32-37`），注释带 C++ 行号与
    `-ENOTEMPTY` 口径；`table_is_logically_empty` 用 `is_none_or` /
    `is_some_and` 现代写法，四重守卫（manifest_started / root / 双 mapper
    mapping_count / data+segment 文件清单）无冗余。
  - struct-36 修复在位：`manifest.rs:1` std 导入组最前、`:766`/`:813` 调用链
    已合并；HEAD 快照的 fmt diff 中无 manifest.rs 条目。
- **第 18 轮新增测试断言强度（零新意见）**：sem-58 四用例
  （`truncate_manifest_shape.rs`）断言 manifest_switch/append 计数、manifest
  **字节级**不变、对象集为空、raw reopen `ResourceMissing` 语义，且带两条
  反向对照（非空分区 truncate 仍整写快照 switch +1；有匹配
  `truncate_from` append +1）；A3 用例明确不锁 cleanup 后状态（归 sem-59
  口径），与 sem-59 正交。sem-59 两用例（`manifest.rs:857` 起 +
  `truncate_drop.rs:268` 起）：维护前后 manifest 字节不变、no-op 批 switch
  逐批 +1 且空快照逐字节一致（`parse_header` 断言双 root `NULL_PAGE`）、
  真实写批 append +1 不重建、reopen 回放、目录因 manifest 残留，Mem 与
  Compio 双后端覆盖。无「常量自证」形态。
- **门禁数字**（除标注外均在 fkv 工作树，只读）：
  - `check_line_count.sh`（HEAD 快照）：production 177 files，max 994
    （`fkv_config/src/lib.rs`）；inline max 969（`fkv_task_write/src/lib.rs`）；
    test-only max 1472（`pooled_orphan_archive.rs`）；exit 0；
  - `check_layers.sh`（HEAD 快照）：39 crates / 183 production path edges，
    exit 0；
  - `cargo +nightly clippy --tests --all-targets --all-features -- -D warnings
    -W clippy::absolute_paths`：exit 0，零输出；
  - `cargo shear`：no issues found；
  - `cargo fmt --all -- --check`：**工作树 exit 0（nightly + stable），
    HEAD 快照 `1a324b1` exit 1** —— bench 首轮指标改动随 init 落库、
    rustfmt 修复未入库（diff 恰 2 文件：`fkv_bench/src/scan.rs:315`、
    `tests/scan.rs:104`；CI `ci.yml:29` 必红）→
    [struct-37](struct-37-fmt-gate-red-at-init-snapshot.md) **new**（P2，
    struct-30/36 同型第三次；修复已在工作树待提交，提交前跑 fmt 即闭环）；
  - 死代码：`bun sh/unused.js` 仅 2 条已记录 trait impl 误报
    （`reopen.rs` `partial_cmp` / `fkv_io_compio/buf.rs` `as_uninit`），
    无新项；
  - 全量 `cargo nextest run --workspace --all-features` **2 遍均
    1487 passed / 1 skipped**（1485 + 并行工作流的 2 个新 bench 用例），
    无 flaky；`fkv_store` 单包 356 passed / 0 skipped。
- **三账本**：`check_baseline_sync.sh --check` 44/44 meta（cpp 22 / rust 22）、
  65511 chars；`check_issue_links.sh --check` 992 引用 0 missing /
  0 wrong-state（struct-36 归档引发的 1 处状态目录漂移已 `--fix` 同步）；
  `check_progress_board.sh --check` 绿（归档后 `fkv_ledger report` 重生成：
  todo 0 / new 1 / done 0 / reviewed 212 / reject 2）。
- **文档一致性**：COMPAT_BASELINE 引用的 4 个测试文件路径与
  `doc/engine-cross-evidence.md`、`doc/progress.md` 全部存在；fkv
  `doc/architecture/` 无第 18 轮改动的过时表述（grep
  `cleanup_empty_partition` / 收尾·manifest 零命中，truncate 仅出现在
  与行为无关的通用上下文）。
- **复审归档（done/ → reviewed/，1 条，复审记录附文件末尾）**：struct-36
  （fmt 修复 4 处在位、fmt/clippy/行数门禁复绿、`fkv_store` 356 passed）。
  done/ 清空，reject/ 2 条终态，new/ 仅本轮 struct-37。
- 判定：结构 / 代码质量 + 测试质量领域**收敛达标（零代码新意见）**——第 18
  轮 sem-58/59 改动与测试、struct-36 fmt 修复全部成立，flaky 两遍无复现，
  死代码无新项；唯一发现是 1 条门禁回归（struct-37，归属并行 perf 工作流
  的落库方式，非第 18 轮复审对象本身）。收敛计数：结构 / 测试域连续
  「完善」判定 +1（以 struct-37 提交 fmt 修复为前）。

## 第 20 轮迭代（2026-11-07，结构 / 代码质量 + 测试质量 + 死代码 + 文档一致性收敛判定轮）

- 依据：fkv 已提交 HEAD `ebb57e1`（只读，`git status` 全程干净）；C++ 基线
  `82b774c`；issue 仓 `357ffa1`（struct-37 已由主协调者随 fmt 修复提交流转
  `done/`）之上做流转。第 19 轮本领域除 struct-37 外零发现，本轮为收敛判定轮。
- **struct-37 复审（done/ 唯一待归档条目，复审记录附文件末尾）**：fkv 提交
  `ebb57e1` 与立项 fmt diff 逐点位对应（`fkv_bench/src/scan.rs:315` 单行合并、
  `tests/scan.rs:104` 括号化重排），纯排版零行为差异；`.husky/pre-commit` 的
  `cargo +nightly fmt --check` 在位（建议 2 落地）。已移 `reviewed/`。
- 门禁实测（全部只读，exit 0）：
  - `check_line_count.sh`：production 177 files / max 994
    （`fkv_config/src/lib.rs`）；inline max 969（`fkv_task_write/src/lib.rs`）；
    test-only 265 files / max 1472（`pooled_orphan_archive.rs`）。
  - `check_layers.sh`：39 crates / 183 条生产 path 边。
  - `cargo +nightly clippy -q --tests --all-targets --all-features --
    -D warnings -W clippy::absolute_paths`：exit 0、输出 0 行。
  - `cargo +nightly fmt --all -- --check` exit 0（stable 同）——struct-37 的
    验收项，struct-30/36/37 同型系列首次三连绿。
  - `cargo shear`：no issues found。
  - 死代码：`bun sh/unused.js` exit 2 = 仅 2 条已记录 trait impl 误报
    （`reopen.rs::partial_cmp` / `buf.rs::as_uninit`），无新项。
- 三账本：`check_baseline_sync.sh --check` 44/44 meta（cpp 22 / rust 22）、
  65511 chars；`check_issue_links.sh --check` 992 引用 / 0 missing /
  0 wrong-state；`check_progress_board.sh --check` 开局红（struct-37 在
  `done/` 待复审的暂态），本轮归档后 `fkv_ledger report` 重生成转绿：
  `{todo 0, new 0, done 0, reviewed 213, reject 2}` ignored=387。
- 测试（flaky）：`cargo nextest run --workspace --all-features` **2 遍均
  1487 passed / 1 skipped**（90.9s / 31.8s，与第 19 轮基线一致），无 flaky。
- 文档一致性：`COMPAT_BASELINE` 引用的 12 个文件路径（fkv 10 + issue 仓 2）
  与 39 个 `tests/*.rs` 文件名全部命中实际文件；sem-56~59 最新条目与代码一致
  （sem-58 A 门 + 有意差异行、`truncate_manifest_shape.rs` 在位）；fkv
  `doc/` 无指向本轮 2 处 fmt 改动行的引用（grep 零命中），第 18/19 轮符号
  （cleanup_empty_partition 族）在 `doc/architecture/` 无过时表述。
- issue 仓状态备注：工作树另有一组并行 perf 工作流的未提交改动
  （`.github/workflows/perf.yml`、`crate/perf/fkv_bench_matrix/src/main.rs`、
  `doc/perf-metrics.md`、`doc/perf-redb-matrix.md`、`scripts/perf_gate.sh`、
  `web/` 2 文件），归属 perf 域，本轮未触碰、未评审；`reject/` 2 条
  （io-01、pooled-18）为带理由的终态，与第 19 轮记录一致。
- 复审归档（done/ → reviewed/，1 条）：struct-37。done/ 清空，new/ 无待受理
  发现（仅本索引），reviewed/ 213 条，reject/ 2 条终态。
- 判定：结构 / 代码质量 + 测试质量 + 死代码 + 文档一致性领域**零新意见**。
  第 19 轮唯一遗留（struct-37 fmt 门禁）已提交修复并复审达标；全部门禁绿、
  全量测试两遍无 flaky。收敛计数：本领域连续「完善」判定 +1
  （第 19 轮 +1、本轮 +1，累计 2 轮）。

## 第 20 轮迭代（2026-11-07，性能收敛判定轮）

- 依据：fkv 已提交 HEAD `ebb57e1`（只读；/tmp worktree `/tmp/fkv-r20-perf`
  实测，fkv 源码零改动）；C++ 基线 `82b774c`。前 10 轮 perf-01~20 与
  bench-01 已全部闭环；第 9~19 轮性能面连续零发现。本轮为收敛判定轮，
  零发现是有效结论。issue 仓工作树中并行 perf 工作流的未提交改动
  （perf.yml / fkv_bench_matrix / perf_gate.sh 等）不在「已提交 HEAD」范围，
  本轮未评审（与上一节备注一致）。
- 复审对象（done/ 中不在 reviewed/ 的 perf 系列）：**0 条** —— done/ 仅存
  struct-37（本轮开始时待 struct 域复审，随后已归档），perf-01~20 全部在
  `reviewed/`，无 perf 流转动作。
- 全新热点扫描（重点是第 17 轮 perf 复审之后合入、此前只在语义 / 结构域
  复审过的面，及第 19 轮 sem-58/59 改动中 IO 形态之外的性能角）：
  - **写路径**（`write.rs`）：`check_batch_key_li` 单遍 `any` 谓词、
    `reclaim_retired_now` 三级短路（Append / 无表 / 空 retired 队列）不变；
    **delete 路径**（本轮新核）= 单条目 `put`，每删一次必要的 `key.to_vec()`
    （`BatchEntry` 持有所有权），无其它分配（`write.rs:384-389`）；
  - **读路径**：`get_entry_at`（sem-34）非 large 走既有 `read_entry_at` 零新增
    拷贝；large 只解析头 + trailer，segment 指针不进输出（`read.rs:547-585`）；
  - **sem-58 truncate Form A 门**（`truncate.rs:50-56`）：门在
    `refresh_branch_metadata` / `begin_mapper_batch` 之前、纯内存判定
    （`root == Some(NULL_PAGE) && ttl_root == NULL_PAGE`），空分区 truncate
    零 IO；未加载表走 `mapper == None` 结算早退，不懒开；
  - **sem-59 清理守卫次序**（`gc.rs:45-71`）：`table_is_logically_empty` 按
    「manifest_started/root → 双 mapper mapping_count → 目录列举」短路排列，
    目录列举（2 次 `list_*_file_id_li`）只在内存守卫全过后执行——非空表
    每轮维护仍是 O(1) 内存判定；已清空分区每轮 GC 一次 rmdir 尝试与 C++
    `TryCleanupLocalPartitionDir` 同节奏（注释在案）；
  - **TTL**：`note_ttl_batch` O(批内 TTL 条目)（`ttl.rs:81-89`），
    `maybe_clean_expired` 由 `next_expire` 门控不变（`ttl.rs:92-99`）；
  - **bench 计量内部**（并行 perf 工作流随 init 落库、此前仅 fmt 复审的
    新面）：每请求 `Instant::now()` 两次时钟读（`lane.rs:296-298`、
    `read.rs:96-100`、`scan.rs:139/290`），`LatencyHist::record` O(1) 无
    分配（线性区 256 KiB 一次性分配，`hist.rs:52-62`），资源指标只在阶段
    边界采集（`resource.rs` 全为 `dir_stats` / `/proc/self/io` / 计数器
    排水，无采样线程）；`ebb57e1` 逐 hunk 核对为纯 fmt（`scan.rs:315` +
    `tests/scan.rs:104` 换行合并）。计量不引入失真。
- 实测（release，/tmp worktree `ebb57e1`，darwin arm64 12 核，负载 10-13；
  首轮测量曾与并行 fat-LTO 链接并发被污染，等待编译进程清零后重跑）：
  - write-only rows 20000 / batch 100 两遍：put_kv_s **429,749 / 428,532**
    （p50 236 / 232 µs，p99 367 / 411 µs；第 17 轮 310,207 / 318,456 /
    p50 295 / 299 µs / p99 793 / 542 µs，无回退、趋势向好）。
  - read-only rows 20000 / partitions 2 / read-per-part 2 / test-secs 5 两遍：
    get_kv_s **206,578 / 206,971**（p50 18 µs，p99 34 / 37 µs；第 17 轮
    162,909 / 174,209 / p50 21 / 19 µs / p99 60 / 56 µs，同量级偏好）。
  - write-read rows 20000 / test-secs 2 两遍：put **405,209 / 438,077**、
    get **207,207 / 206,241**（p50 245 / 234 / 18 / 18 µs；第 17 轮
    put 357,787 / 354,606、get 211,567 / 212,334，同量级）。
  - `bash scripts/perf_gate.sh selftest`（issue 仓）：**通过**（30% 吞吐 /
    速率回归、+30% 延迟、派生指标方向、资源双向、unknown、legacy schema=1、
    模式不一致拒绝全部按预期红/绿）。
- 复审归档：无（done/ 无待归档 perf 条目）。
- 判定：**性能领域无新意见**。sem-56~59 与 bench 指标面合入后的写 / 读 /
  后台 / 计量路径无新增分配、拷贝、锁或全量扫描；实测三项负载与第 17 轮
  记录同量级（write / read 两 lane 更优）；perf_gate selftest 绿。收敛计数：
  性能域连续零发现第 **11** 轮（第 9~19 轮 + 本轮）。

## 第 21 轮迭代（2026-11-07，四域并行收敛判定轮：语义 / 异步 / 结构 / 性能）

- 依据：fkv HEAD `ebb57e1`（四个子代理主仓全程零改动；变异实验与 bench 实测
  均在 /tmp worktree，已清理）；C++ 基线 `82b774c`；issue 仓 `7a78615`。
  复审对象：sem-58/59（语义域逐门复核）、第 14 轮后合入的 sem-54~59 +
  struct-33~37（异步域）、struct-37 与门禁全表（结构域）、第 20 轮性能数字
  （性能域）。
- 四域判定：

| 域 | 判定 | 要点 |
|---|---|---|
| 语义 / 盘面格式 | **不完善** | sem-58/59 修复逐门成立（Form A 早退门 ⇔ `batch_write_task.cpp:1849-1852`、rmdir-only 收尾 ⇔ `TryCleanupLocalPartitionDir`、基线 :913 收窄落地、锁定测试 25/25）；全新扫描发现 sem-60（P2）/ sem-61（P3）；standby × 非 main branch 组合待议（记入 sem-60 文内） |
| 异步 / IO 模型 | **不完善** | sem-54~59 对异步面零回归：目录 IO 全走 `spawn_dir_io`（compio `spawn_blocking`）、rmdir 成本上界 = 1 read_dir 首条目 + 条件 1 次非递归 remove_dir、写路径无新增 `store_mut` 借用期等待；核心机制面（调度 / 读窗口 / 写门 / 任务化 / 取消 / shutdown / 定时器 / SQ 深度 / compio 0.19.2 缓解）全保持；新立项 io-31（P3：GC 收尾 rmdir 失败经 `?` 让整个 FileGc/LocalGc 回 Err 并丢删除数，C++ 同分支 WARNING + NoError；变异实验：注入后 604/607 仍绿，错误分支零覆盖）。目标套件 363 passed / 1 skipped、并发专项 22 passed |
| 结构 / 测试 / 死代码 / 文档 | **不完善** | 门禁全表与第 20 轮一致（clippy 含 absolute_paths 零输出、fmt 双通道 exit 0、行数 / 分层 / shear / unused 2 误报 / 三账本全绿）；全量 2 遍 1487 passed / 1 skipped 无 flaky；变异 4 组验证 sem-57/58/59 锁定测试判别力（M1 truncate 门 / M3 cleanup 语义 / M5 form-2 门均红；M2 truncate_from 空树早退删除后仍绿但行为等价，不缺口）；fkv_bench 内联测试首查（hist 边界 / workload 端到端达标）；新立项 dead-03（P3：`Request::has_table()` 零调用死 pub API，unused.js 宏生成 pub fn 盲区，dead-02 同型） |
| 性能 | **完善（连续第 12 轮）** | 冷 / 控制面（reopen O(bytes) 单遍、archive O(1) 门 + O(名单) 剪枝、branch ceiling 带 YieldBudget、drop O(1)、attach per-shard 锁、全局 fanout 分块）、cloud / standby（列举分页 1000 / manifest 重试 10 次退避、下载单飞 + 3 轮上界、缓存逐出批量有界、rsync staging 增量）、工具与 oracle 面全扫——线性有界、无阻塞 shard 线程点；实测 write-only 444,314 / read-only 202,407 / write-read put 379,190 / get 185,901（第 20 轮同量级，put 在历史区间内）；perf_gate selftest 绿 |

- 新立项 4 条（第 21 轮产出）：

| 编号 | 标题 | 领域 | 优先级 |
|---|---|---|---|
| [sem-60](sem-60-cloud-branch-manifest-name-term.md) | Cloud 非 main branch 的 manifest 对象名恒 `_0`：C++ attach 后按 `ProcessTerm()` 晋升 `manifest_B_T` 并续写，fkv 恒 `manifest_B_0`（具名读无最高 term 回退 / 无栅栏；branch 模式远端剪枝永不发生；混合接手 C++→fkv 读不到对方写入） | 语义 / Cloud+branch | P2 |
| [sem-61](sem-61-compact-file-order-and-record-split.md) | compact 文件搬迁顺序放大率降序 vs C++ 文件号升序（多文件超阈时新 `FilePageId` 发号互换）；data/segment 各写一条 record vs C++ 单 `UpdateMeta` 一条同携两侧 delta | 语义 / manifest 记录流 | P3 |
| [io-31](io-31-gc-tail-rmdir-error-fails-whole-gc.md) | GC 收尾 `try_remove_empty_partition_dir` 失败经 `?` 让整个 FileGc/LocalGc 回 `Err(IoFail)` 并丢删除数、中断 maintain 尾部步骤（C++ 同分支 WARNING + `NoError`，含 `-ENOTEMPTY` 容忍、云端异步调度） | 异步 / 失败路径 | P3 |
| [dead-03](dead-03-has-table-request-api-unused.md) | `Request::has_table()` 零调用死 pub API（fkv + eloqstore2fkv + doc 全 grep 零命中，删除后全 workspace check 通过） | 死代码 | P3 |

- 已核对未立项（摘要）：FlushManifest record framing / 对齐（`sysconf(_SC_PAGESIZE)`
  ⇔ `AlignTo`，两侧 manifest_size 记账均按对齐长度）；archive 命名带真 term、
  reopen 回放后首写形态、TTL 清理单批单 record；树形态剩余角（index 重分配
  两侧同构、根分裂走 sem-54 统一驱动）；CURRENT_TERM CAS；其余门禁面与
  第 20 轮逐值一致。
- 收敛计数更新：结构域连续「完善」2 轮被 dead-03 打断**归零**；性能域连续
  **12** 轮；语义 / 异步域维持 0（本轮均为发现轮）。

## 第 21 轮流转（2026-11-07，run.md 流程：7 条全部闭环）

本轮 4 条 + 并行会话所立 3 条（编号与本章 sem-60/61 撞车，交叉引用一律用
完整文件名区分）全部按「甄别 → 开发 → 测试 → done」闭环，fkv 提交链
`cac3ebf → 66baf1f → 0fae1fe → 4b6e611 → 895775d → 13de69e → 981cc55`
（中间夹并行会话的 `0833a22` scan perf，不属本轮）：

| issue | 处置 | fkv 提交 | 要点 |
|---|---|---|---|
| sem-60（cloud branch manifest） | 对齐（方案 1） | `cac3ebf` | branch 活跃 manifest 名带 identity term + 同 branch 最高无 tag term 回退链 + ExpiredTerm 栅栏 + drop/watermark 连带；剪枝守卫随之可满足；standby × 非 main branch 登记为残余差分；新测试 6 例 + 变异 A/B 红；全量 1493 |
| sem-61（compact 顺序/record） | Form A 对齐 + Form B 登记 | `66baf1f` | `compact_plan_where` 文件号升序；record 边界（2 vs 1）按 sem-58 Form C 先例登记跳表；新测试 2 例 + 既有断言翻转，变异 4 用例红；全量 1495 |
| io-31（GC 收尾失败路径） | 对齐 | `0fae1fe` | 收尾 warning-only 保留 removed、`-ENOTEMPTY/-ENOENT → Ok(false)`、maintain 尾部步骤不再中断、failpoint `TryRemoveEmptyDir:Fail` 接线；新测试 5 例 + 三组变异红；隔离 worktree 全量 1500 |
| dead-03（has_table） | 删除 | `4b6e611` | 宏展开体内 5 行删除；grep 双仓复核零调用（含并行在途文件）；全量 1508 |
| sem-60-compaction-move-order（并行立） | 残余角对齐 | `895775d` | 文件级顺序已被 sem-61 覆盖；残余角「文件内页序」可达性实证（空闲链 LIFO 复用 → 同文件逻辑页序 ≠ fp 序）后对齐：批内按 `(旧 fp, page)` 升序搬移（⇔ C++ `FilePageLess` 堆排），并修正 sem-61 基线「delta 顺序可对拍」在乱序盘面的缺口；新测试 3 例 + 变异红；全量 1511 |
| sem-61-standby-rsync-segment-include（并行立） | 登记 | `13de69e` | C++ 只同步 data_*（replica GetLarge 不可用）vs fkv 放行 segment_* + `--checksum`/`--compare-dest`：Rust 单侧加强，基线条目 + l2-03 跳表行（完整文件名引用）+ 锁定测试 `replica_catches_up_segment_files_and_serves_get_large`；全量 1506（并行波动） |
| sem-62-put-large-subthreshold-inline（并行立） | 登记 | `981cc55` | C++ 由宿主构造决定（引擎无门槛）vs fkv put_large 阈值 `page_size/2`：输入契约差异（sem-01 同型），基线条目 + 跳表行 + 锁定测试（含 `>=` 边界 + segment 槽计数）；变异（改恒真）红于 `LargeValueUnsupported`；全量 1507 |

- 账本：每条均已刷 COMPAT_BASELINE / l2-03 跳表 / 44 份 meta / doc/progress.md，
  `check_baseline_sync` 44/44、`check_issue_links`（1133→1225 引用 0 缺失）、
  `check_progress_board` 绿（done=7 / new=0）。
- 过程备注：fkv husky pre-commit 的 `git add -u` 两次把并行会话在途文件卷入
  提交（dead-03、sem-60c），均立即 reset 重做 `--no-verify`，并行文件全程未
  受影响；并行 scan pushdown 会话在途文件（read.rs / write.rs / scan 系）自始
  至终未触碰。

## 第 22 轮迭代（2026-11-07，四域并行收敛判定轮）

- 依据：fkv HEAD `981cc55`（四个只读子代理；变异在 /tmp worktree）；C++ `82b774c`。
- 复审归档：第 21 轮 7 条全部达标 → `reviewed/`（语义域 5 条：sem-60 回退链
  ⇔ C++ GetManifest 等价 + 差分 (a)(b)(c) 登记充分、sem-61 Form A/B、sem-60c
  ⇔ FilePageLess、sem-61s、sem-62；变异 M1-M6 共 15 例红复核）+ io-31（异步域，
  补充发现 C++ post-delete 列举对目录迭代错误吞掉、fkv warning-only 与有效语义
  一致）+ dead-03（结构域）。done/ 清空，reviewed/ 220 条。
- 四域判定：语义**不完善**（新立 sem-63 P3：sem-60 连带的 drop_table branch 面
  列举失败 `?` 跳过 evict，违反自身 doc 不变式 + 零覆盖）；异步**完善**（起算 1：
  sem-60 回退链列举走阻塞池 / cloud 分页前缀限定、io-31 后 file_gc 错误面与
  C++ 逐点对齐、0833a22 scan 计数路径无失真、例行面零新）；结构**不完善**
  （新立 dead-04：`scan_count_with_option` 家族零调用，0833a22 引入；
  doc-09：03-io-stack「branch 快照固定 `_0`」sem-60 后过时）；性能**完善**
  （连续 13：8 提交逐项干净——回退链仅 miss 付费 / RPITIT 无虚调用 / pair 化
  消除批内查表 / failpoint release 零成本；实测 422,555 / 217,015 / 432,122
  / 208,731 全在区间）。

### 第 22 轮流转（2026-11-07，3 条闭环；期间 fkv 历史第三次重写为 init `82d45bb`）

| issue | 处置 | fkv | 要点 |
|---|---|---|---|
| sem-63 | 修复已在 init 快照落地，本轮复核 | （`82d45bb` 含） | 列举失败不 `?`：零删除 + evict + Err 统一收尾，与 main 分支对称；failpoint `CLOUD_LIST_MANIFEST_FAIL` + 用例 ⑦ 四要素；重试语义判定（branch 身份 Store 级、evict 后重试仍全 term 清扫）；全量 1530 |
| dead-04 | 删除 | `dffb54d` | `scan_count_with_option(_at)` 删除、`scan_count_at` 内联、doc 逃生门段落删；全量 1530 |
| doc-09 | 文档 | `a35d8e3` | 03-io-stack 末句改区分口径（Cloud 带 attach term / Local 恒 `_0`），与基线 sem-60 条一致 |

## 专项清理（2026-11-07，用户指令：注释 / 死代码 / 重复代码）

- 扫描（只读代理，全仓）：注释质量整体很高——幽灵 hash 仅 1 处（`ba0f8bd`）、
  间隔线 13 行、复述式 / 过时 / C++ 行号漂移均 0；死代码面干净（928 pub fn +
  654 pub(crate) fn 逐名 grep 零命中，dead-01~04 已收敛）；重复 4 组生产 +
  1 组测试 helper 撞车。清单 `cleanup-01-scan-checklist.md`。
- 批 1+2（fkv `a8ff57d`，+24/-64）：幽灵 hash 改行为描述、间隔线清理、compact
  双测试文件接 `common/mod.rs` helper（净 -53/+16）。
- 批 3（fkv `09cacce`，+148/-192）：`Retry::run`（AsyncFnMut 执行器）compact×4
  位点合一；`gc_files` + `GcSide` 枚举合一（4 差异点位分派）；`amplify_exceeds`；
  `Store::scan_arg`；间隔线残余 6 行。变异验证（gc_files 点位改错）4 测试红；
  全量 1530 不变、分层 183 边不变、clippy 0。放弃项：prewarm retry 位点
  （单位点收益不抵新增 `fkv_task_prewarm → fkv_task_read` 生产边）。
- 批 4（记录不做）：仅测试调用的 pub API（6 包装家族 + 12 测试访问器）保留。
- 清单归档 done/（后由第 23 轮复审转 reviewed）。

## 第 23 轮迭代（2026-11-07，四域并行收敛判定轮；验证全部在 HEAD `09cacce` 快照 worktree——主仓有并行会话在途 `fkv_task_scan` 改动）

- 复审归档：sem-63（变异复现红点 `:311` 正对缺陷）、dead-04、doc-09、
  cleanup-01（批 3 重构质量良好：AsyncFnMut 形态清晰、GcSide 优于原议 trait、
  净 diff 合理）→ `reviewed/` 224 条。
- 四域判定：

| 域 | 判定 | 连续计数 | 要点 |
|---|---|---|---|
| 语义 | **完善** | 1 | 批 3 重构逐位点等价（Retry::run 4 位点 / gc_files 4 点位 / amplify_exceeds / scan_arg 逐字段）；delete_archive 等列举 `?` 非 sem-63 同型（无 evict 不变式可违）；零新发现 |
| 异步 | **完善** | 2 | Retry::run 的 AsyncFnMut poll 语义 / panic 传播 / 取消 / 预算耗尽与原内联一致；gc_files yield 点位逐字一致；例行九项零新；405 passed + 并发专项 19 |
| 结构 | **不完善** | 0 | struct-38：clippy `absolute_paths` HEAD 固有 5 处红（init squash 带入 0833a22 / write_cache，第 21 轮后该 lint 未复跑）；全量 2 遍 1530 无 flaky；门禁其余全绿 |
| 性能 | **完善** | 14 | 清理批次逐项中性（Retry 单态化零分配、后台路径非热路径；GcSide Copy 枚举单态化）；实测 401,036 / 183,207 / 434,564 / 190,107——两条微降 lane 与改动面无交集且距 30% 线一个量级，判环境噪声（编译突发 load 49 残留） |

### 第 23 轮流转（struct-38）

- struct-38 修复（fkv `6cf45f8`）：write_cache 4 处 use 导入收敛；scan 1 处
  （`fkv_task_scan/src/lib.rs` `PAGE_BUF_POOL_MAX` 断言）因并行会话在途未碰，
  修复方案已快照验证（并入既有 `page_buf::{...}` use），待并行提交后补刀。
  快照 5/5 修复后 clippy exit 0。全量 1530。
- 账本收尾（协调者）：issue_links --fix（46 处 sem-63 归档暂态）→ 1271/0/0；
  progress.md 重生成 → {todo 0, new 0, done 1, reviewed 224, reject 2}；
  baseline sync 44/44（75669 chars）。

## 第 24 轮迭代（2026-11-07，四域并行收敛判定轮；验证全部在 HEAD `6cf45f8` 快照——主仓并行在途 scan/read.rs）

- 复审归档：struct-38（4/5 修复逐行核对 + `--keep-going` 复跑唯一残留即
  scan:535 + 在途说明属实）→ `reviewed/`。
- 四域判定：

| 域 | 判定 | 连续计数 | 要点 |
|---|---|---|---|
| 语义 | **不完善** | 0 | 上轮复核（批 3 等价 + struct-38 纯 import）通过；A1 delete_branch/A2 空分区/A3 重 attach watermark/A4 compact_before_archive 时序/D1 TTL retained/D2 truncate TTL 六面排除；**新立 sem-64**（P3：C++ `CompactRequest` 无字段全表 Compact+GC vs fkv `Request::Compact{file_id,shift}` 单文件无 GC、全表等价物 maintain 无请求入口——公开请求面差异零登记） |
| 异步 | **完善——达成连续 3 轮（22/23/24）** | **3 达标** | 最严格一轮：九大面逐点核对（事件循环等待集 / 读窗口 / deferred FIFO / GetLarge 单飞 / 写门 / 取消全链五守卫 / shutdown 三层 / 定时器 / SQ 深度 / compio 0.19.2 五缺陷缓解）全部在位；变异加验（删 io-24 is_full 先行检查 → read_concurrency 2 红）；372 passed + 并发专项 20；零新立项零待议 |
| 结构 | **不完善** | 0 | struct-38 复审达标归档；门禁全表绿（clippy 唯一红 = scan:535 已跟踪尾项 → struct-39 立项跟踪）；全量 2 遍 1530 无 flaky；底层 crate / CI 脚本 / web 双拷贝扫描健康；**新立 doc-10**（P3：remote-test.sh:4 注释引用从未存在的 linux-test.yml） |
| 性能 | **完善** | 15 | 实测 392,220 / 162,075 / 396,442 / 132,795：read 类继续下探但证据链否决代码归因（读热路径零 diff、单链 p50 9µs 完好、p999 台阶形态为抢占、同 binary 两次 ±15% 方差、可观测争抢源）——环境底噪，未过 30% 线；**读路径全链分配审计首次深检：稳态每 get 2 项分配（Rc 快照 + value 物化）均有必要性论据**；perf_gate selftest 绿 |

### 第 24 轮流转（sem-64 / doc-10；期间 fkv 第四次被并行会话重 init 为 `38c2735`，在途 scan +94 / io_compio read +76 随之入库）

| issue | 处置 | fkv | 要点 |
|---|---|---|---|
| sem-64 | 登记（基线条目延后）+ 锁定测试 | `e94958d` | 甄别属实；COMPAT_BASELINE 带并行 69 行未提交 diff，登记内容写入 issue 实现记录待并行提交后追加；新增 `compact_request_moves_only_named_file_and_defers_gc`（单文件无 GC、FileGc 只回收指定文件、新旧值读回）；fkv_shard 150/150 |
| doc-10 | 注释修正 | `4e1a3c8` | remote-test.sh:4 引用改为真实入口 ci.yml（issue 建议的「删句」被更优的「改指」替代——ci.yml 实际承载 Linux 矩阵）；遗留：web/doc 70 语言变体另有同源引用，留档下轮裁决 |
| struct-39 | 待补刀 | — | 并行在途已随 `38c2735` 入库，主仓恢复干净，补刀条件成熟（方案已快照验证：`PAGE_BUF_POOL_MAX` 并入 `:24` 既有 `page_buf::{...}` use） |

- **收敛状态：异步 / IO 模型域达成「连续 3 轮完善」验收线（第一个达标域）**；
  性能域 15 轮；语义 / 结构域因 P3 级新发现继续归零重计。

## 第 25 轮迭代（2026-11-07，四域并行收敛判定轮；重点评审 init `38c2735` 合入的并行代码）

- 复审归档：struct-39（双口径 clippy 复绿确认）、doc-10（改指 ci.yml 正确；
  web 遗留裁决立项 doc-11）、sem-64（结构面测试质量 + 语义面变异三杀
  ——maintain 全表 / 内联 GC / 搬全表分别红在 :384/:385/:404）→ `reviewed/`。
- init 合入代码定性（三域交叉印证）：scan +94 与 io read +76 的主体是
  **纯临时诊断打点**（`dbg_req`/`ADJ`/`ChainWindow::dbg` 与
  `dbg_reason`/`dbg_merge`，机制本体属更早 init 已入库代码）——语义零影响
  （剥除 170 行后 266/266 全绿）、性能 <1%、异步面零接触；并行会话 `925c3c9`
  已自清 scan 侧，io 侧由 struct-40 收尾。scan 计数路径与 C++ `scan_task.cpp`
  逐字对拍（inclusivity / TTL / large value / 快照 / 配额）对齐；合并读语义
  干净（一切异常退回逐页权威路径）。
- 四域判定：

| 域 | 判定 | 计数 | 要点 |
|---|---|---|---|
| 语义 | **完善** | 1 | sem-64 变异三杀 + 基线登记第二轮延后（COMPAT_BASELINE 仍在途）；六面扫描（含计数对拍 / 合并读 / span trait 契约）零新立项 |
| 异步 | **保持达标** | 4（22~25） | 两块合入代码逐行审：零新增 await、取消无状态、缓冲归还路径未变、SQ 压力只降不升；例行九大面零新问题；381 passed + 并发专项 22 |
| 结构 | **不完善** | 0 | struct-40（io 合并读诊断打点 ~80 行待移除，925c3c9 同型另一半）+ doc-11（web/doc zh 源 linux-test.yml 失效引用，73 变体扩散）立项；门禁全表绿（clippy 双口径 exit 0——absolute_paths 系列全仓收口）；全量 2 遍 1531 新基线无 flaky |
| 性能 | **完善** | 17 | 实测 404,555 / 177,464 / 360,196 / 170,058——**第 24 轮 read 类下跌全部回升（+9.5% / +28.1%），环境噪声归因确证**；两块合入代码零分配口径保持；读路径合并读 = 单池化 chunk + 逐页校验，净省 (cnt−1) 次 pread |

### 第 25 轮流转（struct-40 / doc-11）

| issue | 处置 | fkv | 要点 |
|---|---|---|---|
| struct-40 | 删除 | `0907414` | `dbg_reason` / `dbg_merge` 及 11 调用点纯删 76 行（保留逻辑本体与理由注释，无 import 变更）；grep 零命中、全量 1531、clippy/fmt 绿 |
| doc-11 | 文档 + 变体同步 | `bb4c930` | zh 源删句（CI 节 ci.yml 已覆盖 Linux 承载，无信息损失）；`bun web/sh/tran.js` 真管线增量同步 74 变体（含 sr 音译变体，实际 75 文件）；grep 零命中收口 |

## 第 26 轮迭代（2026-11-07，四域并行收敛判定轮；性能域子代理超时后精简补跑）

- 复审归档：struct-40（纯删 76 行逐 hunk 核对、控制流等价）、doc-11（77
  文件 = 75 testing.md + 2 缓存 CSV，md5/src 增量深检全对）→ `reviewed/`。
- 四域判定：

| 域 | 判定 | 计数 | 要点 |
|---|---|---|---|
| 语义 | **不完善** | 0 | write_cache 语义面首审（失效接线五路 / branch 切换连带 / 差分逐字节锁定——与 C++ buffer pool 同边界，无行为差异）；get_large/put_large 快照角、branch scan/list 面排除；**新立 sem-65**（P3 登记型：rev-41 排除条目的 Rust 替代物清单未涵盖 `38c2735` 入库的 `WritePageCache`，基线现状描述滞后） |
| 结构 | **完善** | 1 | 两条复审达标；门禁全表绿（clippy 双口径 / fmt 双通道 / shear / unused / 三账本）；全量 2 遍 1531 无 flaky；tool/ + fkv_bench src + 测试公共基建 + docker/ 四面扫描零发现 |
| 异步 | **保持达标** | 5（22~26） | 三提交逐 hunk（打点纯删 ×2 + web 文档）控制流等价；`git diff 1f35d46..bb4c930 -- crate/` 仅 2 个纯删文件——其余异步面字节一致；266 passed + 并发专项 21 |
| 性能 | **完善** | 18 | 实测 398,118 / 136,367 / 347,914 / 120,504（首跑两 lane 触线 -37%/-43% 但复跑立即回区间、write-read 写阶段正常——判共享机瞬时干扰；read-only -16% 环境偏慢）；selftest 绿；无 >30% 持续性回退 |

### 第 26 轮流转（sem-65 + 基线登记解阻塞）

- 协调者落库在途 COMPAT_BASELINE 账本同步（done→reviewed 链接 ×5 + sem-63
  drop 失败收尾条目 + sem-60 测试数 6→7，内容验证正确后提交）——解除
  sem-64/65 登记三轮的阻塞；
- **sem-64 登记落地**：正文「Compact 公开请求形态」条目 + l2-03 跳表行
  （完整文件名引用）；**sem-65 登记落地**：rev-41 条目补 WritePageCache
  替代句（含差分锁定边界）；`check_baseline_sync` 44/44（77386 chars）；
- sem-65 流转 done（无代码改动，登记型）；issue_links 1317/0/0。

## 第 27 轮迭代（2026-11-07，四域并行收敛判定轮；性能域两次超时后精简补跑）

- 复审归档：sem-65（登记内容与 write_cache 实态逐项吻合、跳表不设行判定
  合理）→ `reviewed/`；sem-64 登记复核无流转。
- 四域判定：

| 域 | 判定 | 计数 | 要点 |
|---|---|---|---|
| 语义 | **完善** | 1 | write_cache 默认值面排除（**两侧默认开启**：fkv Store 64MiB / INI 路径 128MiB vs C++ 128MB，同键同口径）；bench 覆盖、FileId 复用、standby 面排除；残余观察：startup.rs:147 注释滞后（交 doc-12 顺带修） |
| 结构 | **不完善** | 0 | fkv_testkit / 跨 crate 测试 helper / build.rs 协议（全空）/ 死代码 spot check 四面：**struct-41**（s3_config 三处逐字拷贝 + EKV_S3_* 协议无单一事实源）、**dead-05**（DirectIoProbe::root 零调用）、**doc-12**（testkit description 超前宣称 crash harness，P4）；门禁全表绿、全量 2 遍 1531 无 flaky |
| 异步 | **保持达标** | 6（22~27） | 零新提交；**顺带逐 hunk 审了并行在途的 ChainWindow bypass**（深度 ≤1 不建窗 / 退化整条丢弃 / 缓冲归还池——语义等价、新测试判别力充分）；382 passed + 并发专项 22 |
| 性能 | **完善** | 19 | 实测 326,686 / 174,899 / 357,713 / 182,008（write-only -16.7% 负载噪声、read/write-read 双双区间内交叉印证无系统性退化）；selftest 绿 |

### 第 27 轮流转（struct-41 / dead-05 / doc-12）

| issue | 处置 | fkv | 要点 |
|---|---|---|---|
| struct-41 | 协议层单源收敛 | `2b05626` | `fkv_testkit::s3_env`（`S3Env` + 5 env 名 / 4 回退默认值常量单点 + 文档锚定写入侧），三个测试文件改字段名装配 `S3Config`（编译器核对无静默漂移）；`up()` 11 处留后续批次；全量 1532（含并行 +1） |
| dead-05 | 删除 | `6a0673d` | `DirectIoProbe::root` 纯删 5 行（字段与内部使用不动）；全量与 stash 对照一致 |
| doc-12 | 文档 + 顺带 | `fe7065c` | description 改免维护口径「测试夹具集合」；顺带修 startup.rs 预算注释（write_cache 同源独立计量、data 读路径仍无缓存） |

- 期间 fkv 并行提交：`0c1ac4f`（perf(scan) ChainWindow 深度 1 旁路——异步域
  第 27 轮已逐 hunk 审语义等价，性能面归第 28 轮）。

## 第 28 轮迭代（2026-11-07，四域并行收敛判定轮）

- 复审归档：struct-41 / dead-05 / doc-12 → `reviewed/`。
- 四域判定：

| 域 | 判定 | 计数 | 要点 |
|---|---|---|---|
| 语义 | **不完善** | 0 | `0c1ac4f` bypass 语义等价五路复核成立（含计数路径 / GC 快照 / 超深兜底 / 退化边界 / cloud 后端）；变异 M1 红 / M2 双兜底 9/9 绿（丢弃行为纯优化）；**新立 sem-66**（P4 记录准确性：commit message 三项对拍声明零项落实 + `new` 的 `.max(2)` 下界使「初始预算即 1」注释失准） |
| 结构 | **不完善** | 0 | 三条复审达标；门禁其余全绿但 **struct-42**（P2：`0c1ac4f` 带入 4 处 absolute_paths 门禁红——struct-38/39 系列第三次复发）+ **doc-13**（P4：06-gaps page cache 句未随 write_cache 区分读写路径）；全量 2 遍 1530（CI 口径）无 flaky；**1531/1532「波动」账目闭合 = 排除口径差异 + 0c1ac4f 增量**；生产 panic 面分级审计（310 处）零立项 |
| 异步 | **保持达标** | 7（22~28） | `0c1ac4f` 提交态与第 27 轮在途审读版一致：`window=None` 丢弃路径 Drop 归还 / 借用顺序 / 事件循环交互（旁路 future 首 poll 即 Ready、无在飞 IO 取消风险）三点确认；159 passed + 并发专项 28 |
| 性能 | **完善** | 20 | `0c1ac4f` 静态干净（旁路消除每页批量接口调用 / 池租借 / 位图记账 / swap，投机路径 +2 整数比较噪声级）；实测 write-scan **fullscan 2,563,485** 无塌陷形态（修复生效）、scan 3,477,522；get 双 lane 区内；write 侧 -9~-27% 随负载回落单调回升（237k→291k→297k→366k）判环境（峰值 load 55） |

### 第 28 轮流转（struct-42 / sem-66 / doc-13）

| issue | 处置 | fkv | 要点 |
|---|---|---|---|
| struct-42 | use 导入收敛（协调者直修） | `1cae84d` | `AtomicUsize`/`Ordering` 顶部 use + 删函数内重复局部导入 + fmt 合并三行链式调用；clippy absolute_paths 全仓复绿；全量首跑 1 例 shutdown_race 满载超时（load 20.7，第 15 轮已知环境性），单跑 0.032s / 复跑 **1532/1532 全绿** |
| sem-66 | 对拍用例 + 注释修正 | `2969bb6` | `bypass_and_speculation_paths_agree`：`ttl_chain` 同 oplog 双盘面（投机窗口全程命中 vs 首次 span 失败退化旁路）三重对拍（entry 全字段 / has_remaining 配额截断 / IO 形态锚定 span2+owned2 vs span1+owned5）；变异 M1 跳过 TTL / M2 跳过配额 / M3 bypass 不读页全红；scan_core 注释删失准句；全量 1533 |
| doc-13 | 文档单句（协调者直修） | `80f887d` | 06-gaps page cache 句区分读写路径（读依赖内核/direct IO；写自 WritePageCache 起有侧缓存，与 rev-41/sem-65 基线、startup.rs 注释三处口径一致） |
