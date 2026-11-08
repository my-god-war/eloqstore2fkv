# fkv → C++ eloqstore 全量对齐路线图

- 生成日期：2026-11-06
- 边界（用户决定）：**验收边界 = 对齐 C++ 全量**（不再以 L1 为交付边界）
- C++ 基线：`eloqstore@82b774c62728b023b03c6e5e84e69416ffec9d74`（`COMPAT_BASELINE` 首行 pin）
- 依据：三份只读盘点（数据面格式/压缩、Cloud/standby/宿主、内存缓存/引擎模型）
  逐条读了 C++ 与 Rust 两侧源码，行号见各节；**未做跨实现差分执行**（本机 darwin 无法构建 C++）
- 说明：本文是**差距清单 + 排期**，不是已完成的结论。凡与 `COMPAT_BASELINE`
  现行排除项冲突之处，以本文为准并需回写基线。

## 0. 结论摘要

当前实际等级：**L1**（Local + append/pooled + 无压缩；引擎级 V4/V3 差分已执行）。

按「全量对齐」为边界，任务规模是**三个量级之上**的：本地数据面格式已基本对齐
（§1），但**压缩、cloud/standby 的完整语义、宿主集成、内存/调度模型**四块都有
成片差距，且其中相当一部分差距**不是「写代码」能解决的**（见 §5 永久排除项的判定方法）。

路线图按「L2 → L3 → L4」三阶段组织：

| 阶段 | 内容 | 本阶段完成时「功能完整」的含义 |
|---|---|---|
| **L2** | Local + Cloud + Standby 三模式的数据面互通：对象命名 / 盘面格式 / 恢复点 / term fencing / 归档 | 同一 store 目录（含云对象集合）可在两实现间双向读写，含压缩开启、branch/archive、segment 级 large value、跨重启续写 |
| **L3** | 行为语义对齐：并发/调度/内存上界/背压/观测面 | 同配置下两实现的行为可对账（限流口径、IO 次数、指标标签、内存峰值机制） |
| **L4** | 宿主与资源模型对齐 | C++ 的宿主集成设施在 Rust 侧有等价或明确的边界声明（多数为**永久排除**，见 §5） |

## 1. 数据面格式与压缩

| # | 能力 | C++ (file:line) | Rust 现状 | 性质 | 互操作 | 阶段 |
|---|---|---|---|---|---|---|
| F1 | zstd 实现与版本 | 系统 `libzstd`，版本**不 pin**（`CMakeLists.txt:62`；Ubuntu 24.04 的 `libzstd-dev` 为 1.5.5） | `mzstd 0.2.11`（纯 Rust，与 zstd 1.5.7 逐字节一致，`zstd_parity.rs` 142 + 42 例锁定）+ `zstd-sys 2.1.2+zstd.1.5.7` 仅用于 `ZDICT_trainFromBuffer` | 语义不等价（已登记 l2-07：跨实现帧逐字节仅在 C++ 链接 1.5.7 时成立，属部署约束；见 `COMPAT_BASELINE`「压缩帧与字典训练的逐字节等价边界」） | 否（帧跨版本可解） | **L2**（跳表 l2-07） |
| F2 | term bump 条件 | append 且 `DeriveStoreMode != Local && expect_term != 0 && manifest_term != expect_term`（`replayer.cpp:350-362`）；`expect_term` 来源只有 `ProcessTerm()`，而 Local 强制 shard term 0（`eloq_store.cpp:929-932`）⇒ 第一个条件冗余 | `self.term != 0 && replay.term != self.term` + 「bump 撞 branch 上限则保留原高水位」保护 | **判据等价**；差异只在 Local 是否采纳宿主 term（C++ 丢弃、fkv 采纳 ⇒ 文件名带 `_<term>` 且跨 term 重开 bump）+ C++ 自身 Standby 下 data/segment 口径分叉。已登记（l2-05a，保留 fkv 口径） | **是**（续写布局） | **L2**（跳表 l2-05a） |
| F3 | reopen 时 `max_fp_id` 抬升 | 只用 manifest 首 varint，不按 mapping 抬升（`replayer.cpp:151-152`），靠写侧不变量 + `assert(using_fp_ids.back() < max_fp_id_)`（`:388`） | `max(manifest.max_fp_id, max_mapped_fp_id+1)`（`fkv_recovery/src/lib.rs:99-132`） | **Rust 单侧加固**（已登记 l2-05b）：水位低于被引用页时分配器会交出仍被引用的页；写侧不变量成立时为恒等变换，两侧水位一致 | **是** | **L2**（保留取 max） |
| F4 | branch + large value 的 segment 段 | `CreateBranch` 的 `Snapshot` 不传 segment（`root_meta.h:119-130`） | 恒写该段（`fkv_manifest/src/lib.rs:260-263`） | 有意差异（已登记） | **是**（该组合） | L2（**保留 Rust 行为，把该组合排除出对拍**） |
| F5 | `MappingTbl::resize` 回填 | 只填新 chunk，旧 chunk 尾 stale 槽保留（`page_mapper.cpp:266-300`） | 显式填 `[old_len, len)`（`mapping_tbl.rs:128-163`） | 有意差异（已登记 core-29，l2-06 已回写 `COMPAT_BASELINE` 对拍跳表） | **是**（manifest 字节） | L2 |
| F6 | `manifest_limit = 0` 语义 | 恒写 snapshot（`write_task.cpp:806-807`） | 恒写 snapshot（l2-06 已对齐：去掉 `manifest_limit != 0` 前置，`fkv_store/src/manifest.rs:247`） | 已对齐（l2-06） | 否（非默认） | L2 |
| F7 | 字典训练抽样 / 补足 MIN_SAMPLES | `std::mt19937_64` 重抽直到达标（`compression.cpp:229-244`；`random_device` 播种不可复现） | 整批一次加入，不重抽（`fkv_compress/src/lib.rs:260-273`，确定性） | 语义不等价（已登记 l2-07：`dict_bytes` 不保证逐字节一致、不纳入对拍 oracle；见 `COMPAT_BASELINE`「字典训练抽样」条） | 否 | L2（仅当要求压缩盘面逐字节等价；跳表 l2-07） |
| F8 | `GetMapper` 1:1 | 独立函数，append/pooled 两分支 + cloud term bump + branch range 过滤（`replayer.cpp:305-430`） | 逻辑分散在 `install_replay_state` + `update_stat_by_scan` + `reuse_floor` | 有意简化（已登记 sto-52） | **是** | L2（依赖 F2/F3 定案） |
| F9 | 压缩保留决策比较口径 | `written >= input.size()`（`compression.cpp:54,317`） | `prefix_len + compressed.len() >= input.len()` | 有意简化（已登记） | 否 | L3 |
| F10 | 字典加载校验 / 训练失败退化 | 无长度上限；失败仍走空字典压缩（`compression.cpp:164-172,266-292`） | 拒 >16 KiB；失败恒 raw（`fkv_compress/src/lib.rs:202-213,233-249`） | 语义不等价（未登记） | 否 | L3 |
| F11 | legacy manifest 缺 branch 段的默认名 | 保留空 `branch_name`（`replayer.cpp:189-206`） | 回退 `"main"`（`fkv_recovery/src/lib.rs:133-136`） | 语义不等价（未登记） | 否 | L3 |
| F12 | `data_page_size` 校验基 | `sysconf(_SC_PAGESIZE)`（`eloq_store.cpp:154`） | 固定 4096（`fkv_config/src/lib.rs:506-513`） | 语义不等价（未登记） | 否 | L3 |
| F13 | 四分页切分点 / branch file id 预留 / `data_file_size` u32 截断 | 见 `COMPAT_BASELINE:87-89,151-158,245-252` | — | 有意差异（已登记） | 否 | 保持登记 |

**已逐条对照确认对齐的部分**（页头/页类型、XXH3 checksum 覆盖、LEB128 varint、
restart 间隔、entry 容量判定、overflow 指针上限与分组、large value content 布局、
segment 文件布局、data/segment/manifest/CURRENT_TERM 命名、manifest record framing 与
页对齐、manifest checksum mixer、snapshot/log 段顺序、`file_range_li` 序列化与二分、
撕裂尾截断、`ReplayLog` delta、分配器统计、段 mapper 恒 Append、pooled 空闲池
branch 过滤、压缩在 page 层的启用面）见盘点原文；本表只列**未对齐**项。

## 2. Cloud / S3 / standby / 多 shard

| # | 能力 | C++ (file:line) | Rust 现状 | 性质 | 阶段 |
|---|---|---|---|---|---|
| C1 | **`max_cloud_concurrency` 额度模型** | **每 shard 一个额度**（`async_io_manager.cpp:4370-4381` + 每 shard 一个 `obj_store_` `async_io_manager.h:1576`），全局共享的是 HTTP 线程池（`eloq_store.cpp:758`）⇒ 整机上界 ≈ `shard_num × max_cloud_concurrency` | 按实例数**均分**（`fkv_shard/src/backend.rs:74-104`）⇒ 整机 ≈ `1×` | **语义不等价 + 基线描述错误**（`COMPAT_BASELINE` 写成「共用一个 AsyncHttpManager」，与 C++ 代码不符） | **L2（先改基线，代码可保留）** |
| C2 | standby 镜像内容校验 | 只查 `manifest.tmp` 存在（`standby_service.cpp:1066-1073`） | `validate_manifest_bytes` 完整 record 链 + 重列比对 + `verify_same`（`fkv_standby/src/lib.rs:517-570,599`） | Rust 单侧加强（未登记） | **L2（登记）** |
| C3 | 冷态孤儿归档 `reopen_archive` | C++ 能读（`GetMapper` 覆盖孤儿归档页） | large value 配置下 fail-stop `ResourceMissing`（`fkv_store/src/archive.rs:213`，pooled-13 在修） | 语义不等价（已立案） | **L2** |
| C4 | standby 孤儿数据文件清理 | C++ 有已知缺陷 + TODO（`reopen_task.cpp:118-128`） | 需核实是否周期触发 `GcScope::Local` | 需核实 | L2 |
| C5 | cloud manifest 采纳栅栏 / `ReopenArchive` term 选择 / 归档剪枝 / `DeleteBranch` 删除面 | 见 `COMPAT_BASELINE` sem-02/04/27 条 | 已按各自判定实现 | 有意差异（已登记） | L2（**逐条标注「不纳入逐字节对拍」**） |
| C6 | `Global*` fanout 错误聚合口径 | 只留 `first_error_`（`eloq_store.cpp:2030-2050`） | `GlobalReply{ok_num, failed_li}`（更丰富） | 语义不等价（未登记） | L3（登记） |
| C7 | S3 重试次数/退避 | `max_retries_=5`（`object_store.h:95`） | 默认 3 + 指数退避 + 抖动 | 参数面差异（未登记） | L3（登记） |
| C8 | `reserve_space_ratio` / `max_archive_tasks` | `async_io_manager.cpp:6973-6979` / `eloq_store.cpp:1899-1913` | 解析不生效 | 参数面未接线 | L3 |

**已确认对齐**：对象命名全套、整对象 PUT 覆写、term 文件 CAS、manifest 列举无缓存、
凭证/SSL/`cloud_auto_credentials`、云 data/segment GC、被取代 manifest 剪枝、
standby manifest 选择 + `ExpiredTerm` 栅栏、rsync 缺源映射、`clear_local_state`、
`global_reopen` 合并远端∪本地 + `SetClean(true)`、请求路由 `partition_id % shard_num`、
`Global*` fanout 分批与并发。

## 3. 内存缓存 / 引擎模型 / 任务并发

| # | 能力 | C++ (file:line) | Rust 现状 | 性质 | 阶段 |
|---|---|---|---|---|---|
| M1 | **index 页首访 single-flight（WaitingZone）** | 同页并发首访合并为一次 IO（`task.h:246-261`、`task.cpp:492-556`、`page_manager.cpp:706-722`） | **缺失**：`PageCache` 无在飞去重，N 个并发读同一未缓存页各读一次（云端下载层有 `single_flight` 可复用） | 完全缺失（未登记） | **L2** |
| M2 | **compact 协作让出** | 每文件 / 每 256 mapping `MaybeYield`/`YieldToLowPQ`（`background_write.cpp:68-91,172-190,238`） | `fkv_task_compact` **全文件无 `YieldBudget`**，CPU 规划/排序段不让出 | 语义不等价（未登记） | **L2** |
| M3 | 指标标签集（4 个 `global_*` + `unknown`） | `eloq_store.h:40-112` 共 23 标签 | 19 标签（`fkv_metrics/src/lib.rs:127-147`），缺全局操作 | 完全缺失（未登记） | **L2** |
| M4 | gauge：index buffer pool limit / open file count(±limit) / local space limit | `eloqstore_metrics.h:47-53` | 只有 used 类，无 limit | 完全缺失（未登记） | L3 |
| M5 | 各任务类型并发上限（scan/list/prewarm 池） | `task_manager.cpp:20-56,121-153` | 仅读侧 `MAX_READ_IN_FLIGHT=128` + 写侧 `WriteGate`；scan/list/prewarm 无独立上限 | 语义不等价（未登记） | L3 |
| M6 | `max_inflight_write` 的 append 门控 | 仅非 append 生效（`kv_options.h:79`） | 恒生效（并兼作 fdatasync 并发上限） | 语义不等价（未登记） | L3 |
| M7 | index 池预算与 `write_buffer_ratio` 的切分 | 同一 `buffer_pool_size` 按 ratio 预留后切分（`page_manager.cpp:29-44`、`async_io_manager.cpp:301-347`） | index 池独立预算，无 ratio 扣减 | 语义不等价（未登记） | L3 |
| M8 | 大值读内存上界（slab 恒定 vs 瞬态） | slab 段硬限，段不可得即 `yield()`（`task.cpp:259-266`） | `vec![0u8; actual_len]`（`fkv_large/src/lib.rs:248-249`），上界 = 并发×值长 | 资源上界不同（已登记） | L3（登记机制差异；**不追求 1:1**） |

**已确认对齐**：每分区写 FIFO + 全局写并发门、高低优先 + 时间片防饿死
（`max_processing_time_us` 同为 400）、OOM/auto-reopen 重试、`ExpiredTerm` 熔断、
写后 compact 触发（append 门控）、`PendingWrite` 聚合写（`PENDING_WRITE_BUDGET=1 MiB`）、
fsync 顺序（Rust 单侧加强只 fsync 脏文件）、批读批写批大小、RootMeta LRU 上限、
GC/archive/replay 的协作让出、`retain_mapped` 等价 swizzle 复用、fixed buffer 注册与取消归还。

## 4. 分阶段执行顺序（含依赖）

**L2 第一批（先把「基线描述错误」清掉，成本最低、但不做会让后续对拍以错误前提进行）**

1. **C1** 修正 `COMPAT_BASELINE` 的 S3 并发条目：区分「共享 HTTP 线程池」与
   「per-shard 额度」，写明同一配置值两侧整机上界不同（C++ 可达 `N×`，fkv ≈ `1×`）
   及迁移时的缩放建议。**无需改代码。**
2. **C2** 登记 standby 镜像校验的 Rust 单侧加强。
3. **C5** 把 sem-02/04/27 等「有意差异」逐条标注「不纳入逐字节对拍」，避免差分器误判。
4. **M3** 补 4 个 `global_*` 指标标签（或明确登记「全局操作不计入 shard 指标」）。

**L2 第二批（格式与续写布局，必须成组做，单做无意义）**

5. **F2 + F3**（term bump 条件、reopen `max_fp_id` 抬升）——先对齐
   `ProcessTerm()` 取值口径，再改 `fkv_recovery`/`manifest` 的抬升逻辑；
   **l2-05 已结案**：`ProcessTerm()` = 运行任务 shard term，Local 被强制 0 ⇒
   C++ 判据与 fkv 等价，差异改为登记（Local term 口径 l2-05a + 两处 Rust 单侧加固
   l2-05a/l2-05b），不改代码；见 `issue/reviewed/l2-05-term-bump-and-reopen-max-fp-id.md`；
6. **F5 + F6**（`MappingTbl::resize` 回填、`manifest_limit=0`）——manifest 逐字节等价的最后两块；
   **l2-06 已完成**：F6 对齐 C++（恒 snapshot）并加测试锁定，F5 判定保留 Rust 回填 +
   回写 `COMPAT_BASELINE` 跳表 core-29（见 `issue/reviewed/l2-06-mapping-resize-backfill-and-manifest-limit-zero.md`）；
7. **F1 + F7**（zstd 版本 pin + 字典抽样）——仅在要求**压缩盘面逐字节等价**时才做，
   否则登记为「压缩体不保证逐字节等价」；
   **l2-07 已结案并登记**：Rust 侧 `mzstd =0.2.11` / `zstd =0.14.3` / `zstd-sys =2.1.2`
   精确 pin，`fkv_compress/tests/zstd_parity.rs` 实测 mzstd 与 zstd 1.5.7 帧逐字节
   一致（142 + 42 例）；字典抽样差异（C++ `mt19937_64(random_device)` 重抽）
   `dict_bytes` 不纳入逐字节对拍。C++ 侧不 pin libzstd（Ubuntu 24.04 为 1.5.5）属
   **部署约束**、不在本仓可修范围，跨实现帧逐字节仅在 C++ 链接 1.5.7 时成立；
   见 `COMPAT_BASELINE`「压缩帧与字典训练的逐字节等价边界」条与跳表 l2-07；
8. **F8**（`GetMapper` 1:1）——依赖 5；
9. **F4** 定案：保留 Rust 恒写 segment 段，把该组合排除出对拍并在 L2 声明写明；
10. **C3 / C4**（冷态孤儿归档、standby 孤儿文件）。

**L2 第三批（行为可观测面）**

11. **M1** index 页 single-flight（唯一会改变可观测 IO 次数的缺失项）；
12. **M2** compact 接 `YieldBudget`。

**L3**：F9~F12、C6~C8、M4~M8、以及 §1/§3 中标注 L3 的登记类项。

**L4**：见 §5——绝大多数是**永久排除**，产出物是「边界声明」而非代码。

## 5. 建议永久排除项（附判定理由）

判定方法：该项是否为「C++ 宿主/运行时的集成设施」或「换成 Rust 惯用实现后语义
等价」。**永久排除不等于不登记**——每一项都必须在 `COMPAT_BASELINE` 有对应条目
与重开条件。

| 项 | 理由 |
|---|---|
| 单进程多 data store shard（`AttachShard`/`partition_filter`/detach fanout） | 面向 C++ 云上多租户宿主；fkv 以「每进程/每 Runtime 一身份」实现同一能力，**路由口径已对齐**。1:1 需要不存在的外部宿主 |
| `sdk_runtime`（KVCacheManager/Worker 共享内存池 + IPC）、bthread `EloqModule`、`test_utils` | C++ 宿主/测试脚手架，不含用户可见数据面能力 |
| `GlobalRegisteredMemory`（含 ABA 空闲栈、`SetEvictFunc`）、`direct_io_buffer_pool_size`、`pinned_tail_scratch_slots`、`gc_global_mem_size_per_shard` | io_uring registered-memory / KV Cache 模式设施；已核实 `SetEvictFunc` **生产零调用点**，GRM 留空即关闭。fkv 用 per-shard 注册 + 调用方 `FixedBuffer` 覆盖 |
| 用户态页 buffer pool、页级 pin、swizzle、data page cache（含其命中计数） | Rust 由单一 `PageCache`（物理页键 + Clock 引用位）承担，命中即零拷贝；reopen 不再复用缓存（整体清空，性能面有意差异）。`enable_data_page_cache` 在 C++ 默认 false |
| 「buffer pool 耗尽 → `OutOfMem` → 回滚重试」 | 前提是用户态池有硬字节上界；Rust 瞬态分配模型下无该触发源，引入即等于反向引入用户态池。**须登记为「资源上界不同」** |
| `chunk_size` | 已核实 C++ 引擎源码内**无读取点**（仅局部同名变量），是宿主/KV Cache 字段，不是对齐缺失 |
| `coroutine_stack_size`、`segment_compact_yield_every`、`cloud_request_threads` | 调度/线程模型，Rust executor 与单后台线程模型不同，1:1 无意义（Rust 用时间预算替代让出粒度） |
| `init_page_count`、`non_page_io_batch_size` | 纯实现细节旋钮，Rust 无对应结构 |
| standby 本地 manifest 落地名、`ListStandbyPartition` 固定 shard 0 | standby 本地目录内部协议，不在共享对象格式面 |
| 跨实例共享令牌池 | 无数据面影响 |

## 6. 「完善、功能完整」在本文边界下的判据

- **不能**用「连续 N 轮审查通过」作为唯一判据：以全量对齐为边界，`L2` 未完成前
  每一轮都必然是「不完善」，这不是结论而是进度。
- 改为**按阶段判据**：
  - `L2 完成` = §4 第一批 ~ 第三批全部闭环 **且** 引擎级差分在
    `Local / Cloud / Standby` × `append / pooled` × `压缩开 / 关` ×
    `branch / archive / large value / TTL / 跨重启续写` 的矩阵上执行通过；
  - `L3 完成` = §3/§2 的 L3 项全部对齐或登记，且限流/IO 次数/指标三类可对账；
  - `L4 完成` = §5 的每一项在 `COMPAT_BASELINE` 有边界声明 + 重开条件。
- **前置阻塞（环境项，非代码）**：L2 的差分矩阵需要在 Linux（io_uring）上跑，
  且需要 `origin` 可达的、与证据一致的 fkv commit（当前 fkv 历史被反复重写为单条
  `init`，见 `issue/` 收敛小节）。这两项不解决，`L2 完成` 无法被证明。

## 7. 本次盘点的限制（如实声明）

- 三份盘点均为**源码逐条对照**，未执行跨实现差分；C++ 侧在本机（darwin/arm64，
  依赖 io_uring）**无法构建**，所有 C++ 结论来自源码阅读 + 上游测试断言。
- `rev-41` / `rev-44` 的原始判定记录**从未随仓库落盘**（`sem-32` 已确认），
  本文引用它们时以 `COMPAT_BASELINE` 正文为唯一依据。
- 行号基于盘点时的 fkv `79b7dd5` / issue 仓 `678131a`；两仓在被盘点期间均有并发提交。
- 未能核实项：`mzstd` 与 zstd 1.5.7 的字节等价性、C++ Local 下 `ProcessTerm()`
  实际取值、standby 是否周期触发 `GcScope::Local`、`GlobalRegisteredMemory` 的
  实际容量配置（宿主传入，基线仓内无生产值）。
