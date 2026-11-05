# rev-41 MemCachedPage 用户态 buffer pool（slot/pin/驱逐/swizzle）未在 core 实现

## 位置
- Rust: `crate/core/fkv_btree/src/lib.rs:240`（`IndexCache`，仅缓存 index 页）；`crate/core/fkv_mapping/src/lib.rs:24-27`（`is_free_slot` 排除 `SwizzlingPointer`，注释「Rust 目前没有 swizzling 写入路径」）；`crate/foundation/fkv_types/src/lib.rs:58`（`ValType::SwizzlingPointer = 0` 定义但全代码无写入函数）。无 `fkv_cache` crate。
- C++: `include/storage/mem_cached_page.h:26` `MemCachedPage`（`Pin`/`Unpin`/`ref_cnt_:210`、`next_/prev_:230`、`WaitingZone waiting_:223`）；`include/storage/page_manager.h:44 AllocPage`/`:52 EnqueuePage`/`:108 TryRecycleCachedPage`/`:151 Evict`/`:153 RecyclePage`；`src/storage/page_manager.cpp:663 FindPage`（swizzle：`GetSwizzlingHandle`/`AddSwizzling`/`Unswizzling` 见 669-691）、`:776 Evict`（从 `active_tail_` 跳过 pinned 扫描）、`:800 RecyclePage`（对全部快照 `Unswizzling`）。

## 现状
core 层不存在 `fkv_cache`，也没有 `MemCachedPage` 的任何等价结构（全仓 grep 无 `ref_cnt`/`Unpin`/`active_head`/`AllocPage`/`WaitingZone`/`add_swizzling` 等）。唯一的页缓存是 `fkv_btree::IndexCache`：`PageId → IndexNode`、按近似字节 Clock 淘汰、按映射代际 gating，**只覆盖 index 页**。data page 不经任何用户态缓存，读路径每次经 IO 层。`ValType::SwizzlingPointer` 值类型在 `fkv_types` 定义，但没有任何写入/读取 swizzle 的代码路径，`fkv_mapping` 只是防御性把它排除出可回收判定。

## C++ 实现
`MemCachedPage` 是跨表共享的用户态 buffer pool：每个 slot 持 `page_id_/file_page_id_/ref_cnt_/err_/waiting_` 与 `prev_/next_` 双链；读任务下降树时 `Handle` 对页 `Pin`（`ref_cnt_++`），pinned 页不可被 `Evict`；`PageManager` 维护 `active_head_/active_tail_`（MRU→LRU）与 `free_head_` 空闲链，`Evict()` 从 LRU 尾跳过 pinned 找牺牲页并 `RecyclePage`（对所有活跃快照 `Unswizzling` 后归还 free 链）。`FindPage` 先查 swizzle：mapping 槽里若存 `SwizzlingPointer` 直接指向常驻页，省一次 hash；首次加载用 `WaitingZone` 让并发首访同一页只发一次 IO、其余 waiter 挂起复用。`RootMeta.cached_pages_` 记录每表常驻页以便表级驱逐。

## 差异与风险
- **pin/ref_cnt 缺失**：Rust 靠单线程借用规则（`get` 返回 `&IndexNode`，`insert`/evict 走 `&mut self`）替代 pin，无法在 `.await` 期间跨任务持有页，也无法表达「下降途中禁止驱逐」。
- **WaitingZone 缺失**：并发首次加载同一 index/data 页不去重，每个请求各自发 IO → 冷启动 / reopen 预热场景读放大。
- **swizzle 缺失**：mapping 值永远只存 `FilePageId`，无法让热页绕过缓存查找直取常驻内存，C++ 的读快路径无对应实现。
- **data page cache 缺失**：C++ `enable_data_page_cache` 下 data 页也常驻；Rust 无。
- **全局页缓存内存预算缺失**：C++ 由 `PageManager` 的 `IsFull()/Evict()` 统一约束 buffer pool 字节上限；Rust 只有各 `IndexCache` 自身 + `CacheLedger` 约束 index 页，data 侧无常驻故无预算。
上述在「单线程 + append-only」模型下不影响**正确性**（结果与 C++ 一致），属功能未实现 / parity 缺口；但对吞吐、读放大、内存可控性是实质差距。

## 对齐建议
若以 parity 为目标：在 core 新增 `fkv_cache`，实现 `MemCachedPage`（`page_id/file_page_id/ref_cnt/err/waiting/next/prev`）、PageManager 式 `alloc/enqueue/evict/try_recycle`，并在 `fkv_mapping` 让 `ValType::SwizzlingPointer` 进入写入路径（`AddSwizzling/GetSwizzlingHandle/Unswizzling`），`IndexCache` 与 data 页统一到该 pool。若确定不做用户态 data cache / swizzle，应在 `COMPAT_BASELINE` 显式登记为范围外，避免被误判为「已对齐」。

## 严重度
P1

## 核验记录
- 通读 `fkv_btree/src/lib.rs`：仅 `IndexCache`（Clock+generation，index 页），无 pin/ref_cnt/active list/swizzle。
- 通读 `fkv_mapping/src/lib.rs`：`ValType::SwizzlingPointer` 仅在 `is_free_slot` 被排除，无任何 swizzle 读写函数。
- 全仓 grep（排除 `_cpp`/tests）：`ref_cnt`/`Unpin`/`active_head`/`AllocPage`/`WaitingZone`/`add_swizzling`/`data_page_cache` 均无实现命中；`FilePagePool`（mapping:442）是 Pooled 文件页 id 空闲链，`fkv_io_compio` 的 `slot_li` 是 io_uring 槽状态，均非页缓存。
- C++ `page_manager.cpp` 确认 `FindPage` swizzle（663-729）、`Evict`（776）、`RecyclePage`（800）真实实现。结论：功能未实现（非实现不正确）。

## 甄别（2026-11-05 r6）

**部分采纳，P1 → P2，范围收窄。** 事实全部核实属实，但五条「差异与风险」中三条的
parity 框架不成立（C++ 侧默认配置下同样没有 / fkv 已有等效物），仅一条是现时缺口，
一条暂缓。逐条复核如下。

### 证据核实（file:line 逐处亲验）

- C++ 全部引用属实：`mem_cached_page.h:26`（`MemCachedPage`）、`:210` `ref_cnt_`、
  `:223` `waiting_`、`:230-231` `next_/prev_`；`page_manager.h:44/52/108/151/153`；
  `page_manager.cpp:663`（`FindPage` swizzle 首查 → `AddSwizzling` 占位 → detached
  时 `waiting_.Wait`，确为 single-flight 语义）、`:776`（`Evict` 自 `active_tail_`
  跳过 pinned）、`:800`（`RecyclePage` 全快照 `Unswizzling`）。
- **作用域澄清（关键）**：该 buffer pool 是**每 shard 生产路径常开**（`shard.h:293`
  `PageManager index_mgr_` 成员，全部 read/write/scan task 经 `shard->IndexManager()`
  → `FindPage`），**不是 Mem 后端测试专用**；但其中 **data 页缓存是默认关闭的可选项**
  （`kv_options.h:58` `enable_data_page_cache = false`；关闭时 `task.cpp:119` 直接走
  `LoadDataPageStorage` 每读一次 IO，与 Rust 行为同构）。即常开部分 = index 页池。
- Rust 引用属实：`fkv_btree/src/lib.rs:240` `IndexCache`（仅 index 页）；
  `fkv_mapping/src/lib.rs:22-27`（`is_free_slot` 防御性排除 swizzle）；
  `fkv_types` `ValType::SwizzlingPointer` 无写入路径。`fkv_cache` crate 不存在
  （`issue/done/33-orphan-crates.md:87` 已勘误，core-13/core-14 归口 issue 亦不存在，
  本主题此前无任何归口 → 不构成重复立项）。

### 价值评估（逐条裁定）

| 原文主张 | 裁定 | 依据 |
|---|---|---|
| pin/ref_cnt 缺失 | **暂缓** | 当前正确性由借用规则保障（`get` 借不跨 `.await`，中途被逐出则重查，自愈）。是性能项非正确性项；eng-33a（todo）的「COW/pin：27+core-11」依赖属**映射快照层**（27b 已落地代际键），与本条页级 pin 不同层，互不替代。写并发落地且实测出现下降途中驱逐重读再做。 |
| WaitingZone 缺失 | **采纳（唯一开发项）** | 非未来时态：读并发窗口已落地（`fkv_shard/src/lib.rs:716-723` 注释「spawn 成独立任务，进入读并发窗口」，`submit_read_li:1957`），并发冷访同一 index 页各自 miss、各自发 IO，冷启动 / reopen 读放大**今天存在**。 |
| swizzle 读快路径缺失 | **不采纳** | swizzle 是 C++ `FindPage` 的内部机制（mapping 槽直指常驻页省一次 hash）；Rust `IndexCache`（PageId 键 + 代际门控）提供同等「热页零 IO」收益，残差仅一次哈希查找。reopen 时的 swizzle 复用语义已由 eng-67（done）等价落地（`retain_mapped` + `(branch,term,fpid)` 身份校验）。 |
| data page cache 缺失 | **不实现，登记偏离** | C++ 默认 `enable_data_page_cache=false` → 默认配置两侧都无用户态 data 缓存；Rust 默认 buffered 反而多一层内核 page cache（io-21 已决策分层：`with_direct_io(true)` 时两侧同为直读）。注意 `fkv_config/src/lib.rs:175` 该旋钮**解析但无任何消费者**，登记时应一并注明。 |
| 全局页缓存预算缺失 | **陈旧，不成立** | eng-69/69b/69c（done）：每表 `index_cache_bytes` + `CacheLedger` 聚合预算 + 旋钮已落地；`fkv_store/src/lib.rs:312` 注释明确「对齐 C++ `buffer_pool_size` 中划给 index page 的部分」。data 侧无用户态缓存（同 C++ 默认），故无待预算对象。 |

### 开发计划

1. **IndexCache 首访 single-flight**（WaitingZone 等效，P2）：miss 路径按 `(tbl, page_id)`
   登记 in-flight 项，后到者挂起复用首次 IO 结果。验收：并发冷批同键域读，IO 记录器
   断言每个 index 页仅 1 次读 IO。
2. **COMPAT_BASELINE 登记偏离**：新增条目「用户态 buffer pool / swizzle / data page
   cache 不做 1:1：index 侧由 `IndexCache`（Clock+代际+CacheLedger）替代，data 侧由
   内核 page cache（默认 buffered）/ 直读（`with_direct_io`）替代（io-21）」；
   `enable_data_page_cache` 死旋钮注明「解析不生效」，保持 INI 面兼容（sto-57）。
3. **暂缓项**：页级 pin / 统一 `fkv_cache` 池——阻塞于 eng-33a 写并发 + 实测证据
   （下降途中驱逐导致的重读放大），届时再评估，不在本条开工。
4. **明确不做**：运行时 swizzle 快路径；用户态 data page cache。

### 热点文件

- `crate/core/fkv_btree/src/lib.rs`（`IndexCache` miss 路径：single-flight 落点）
- `crate/engine/fkv_task_read/src/lib.rs`（index 回填点，`:246` 附近）
- `crate/engine/fkv_shard/src/lib.rs`（读并发窗口 `:716-723`，验证场景入口）
- `COMPAT_BASELINE`（偏离登记）
- C++ 对照：`src/storage/page_manager.cpp:663-829`、`src/tasks/task.cpp:119-149`、
  `include/kv_options.h:58`

### 严重度修订

P1 → **P2**：默认配置下无正确性 / 行为差距（结果与 C++ 一致，原文亦承认）；唯一
开发项是吞吐优化（首访去重），且已被 eng-67 / eng-69 系列消化大半。

> 认领（2026-11-05 03:20，本会话子代理甄别+开发中）。lane：crate/core/*（fkv_btree/fkv_mapping/fkv_types，可新建 fkv_cache）；若甄别结论需要改 fkv_store/fkv_shard/fkv_io*（并行热点），只做登记路径不动热点代码。

## DONE（2026-11-05）

判定：**B 路径——逐项登记有意偏离，零代码实现**。r6 甄别采纳的唯一开发项
（IndexCache 首访 single-flight / WaitingZone 等效）本轮不落地：集成点越 lane，
见下文第 3 条。r6 五项裁定经一手证据逐处复核，全部维持。

### 逐项判定（一手证据）

1. **pin/ref_cnt：不需要（暂缓维持）**。C++ pin 解决多线程「使用中页被逐出」
   （`mem_cached_page.h:207-212` 注释明示 `ref_cnt_` 语义；`Evict` 跳过 pinned，
   `page_manager.cpp:776-795`）。fkv 单线程下借用规则已覆盖：
   `IndexCache::get(&self)` 返回 `&IndexNode` 的借用仅在 `descend_step` 内即刻
   `seek` 用掉、不跨 `.await`（`fkv_task_read/src/lib.rs:321-350`；eng-32a 已核验
   RefCell 借用不跨 await）；驱逐只发生在同任务 `insert` / `trim_to_budget` 的
   `&mut self` 内，被借用的页不可能被逐出。读请求无跨请求持页：data 页读入
   调用方自有 `PageBuf`、index `seek` 返回 owned `PageId`；27b 读跨 `await` 持
   `MappingSnapshot`（映射层，非页引用）。「读请求跨 await 持页的真实场景」当前
   不存在。
2. **swizzle：不做**。`IndexCache::get` 为一次 gxhash `HashMap<u64>` 查找，每树层
   一次（实际 2~4 层，`MAX_TREE_DEPTH=64` 仅为上界）；swizzle 只省这一次哈希，
   却要求 `SwizzlingPointer`（`fkv_types/src/lib.rs:58`，现无任何写入路径）进入
   mapping 可写值域，波及 `is_free_slot` 回收判定（`fkv_mapping/src/lib.rs:26`）、
   恢复扫描过滤（`fkv_recovery/src/lib.rs:110`）、reopen 缓存复用与 GC retained
   计算——复杂度不成比例。reopen 的 swizzle 复用语义已由
   `IndexCache::retain_mapped` + `(branch, term, fpid)` 身份校验等价落地
   （`fkv_btree/src/lib.rs:447-470`，eng-67 done）。
3. **WaitingZone（首访 single-flight）：真实收益成立，本轮转登记不实现**。
   收益属实：eng-32b 已把读请求 spawn 成独立 compio task（`fkv_shard/src/
   lib.rs:716-723` 注释、`submit_read_li:1974` 两阶段 spawn），≤`MAX_READ_BATCH`
   =32 条在飞共享同表 `RefCell<IndexCache>`，冷缓存下并发首访同一 index 页确实
   各自 miss、各自发 IO。但（a）放大有界：每冷窗口 ≤ 32 × 树深（2~4）次 4 KiB
   index 页读，且默认 buffered IO 下重复读命中内核 page cache（syscall+拷贝级
   成本），`with_direct_io(true)`（io-21）才是真设备读；（b）无实测证据：bench
   冷读链已落地（perf-71/73：reopen 后随机 key 并发读）但只有延迟/吞吐指标，
   无 IO 计数，未观测到放大超阈值；（c）集成点越 lane：single-flight 必须
   包住 miss 路径的 IO——改动点为 `fkv_task_read/src/lib.rs:321-350`（miss →
   `io.read_page` → `insert` 回填是唯一 index 回填点），验证入口在 fkv_shard
   读窗口，两者均不在本会话 lane（core/fkv_btree、fkv_mapping、fkv_types）内；
   只在 fkv_btree 放 single-flight 数据结构而无调用点改造 = 零调用死代码。
   重开条件：写并发（eng-33a）落地后加 IO 计数器实测单冷窗口重复 index 读
   显著超理论下限。
4. **data page cache：不做**。C++ `enable_data_page_cache` 默认 `false`
   （`kv_options.h:58`，注释明示 default 每读走存储；关闭时 `task.cpp:119` 直接
   `LoadDataPageStorage` 每读一次 IO）→ 默认配置两侧同构，不存在「C++ 有而
   fkv 无」的默认行为差。fkv 默认 buffered IO 反多一层内核 page cache；io-21
   已决策 `with_direct_io(true)` 时两侧同为直读。Rust 侧
   `fkv_config/src/lib.rs:175` 该旋钮解析但全仓无消费者（死旋钮），为 INI 面
   兼容保留（已在 §8.1 行内注明）。bench 冷读路径无 data 页读放大记录
   （perf-71 复审：点读冷路径 = 每读 1 次 data 页 IO，与 C++ 关缓存行为一致）。
5. **全局字节预算：不构成偏离**。eng-69/69b/69c（done）已落地每表
   `index_cache_bytes` + `CacheLedger` 聚合 + `index_cache_total_bytes` 旋钮
   （`fkv_store/src/lib.rs:309-323` 注释「对齐 C++ `buffer_pool_size` 中划给
   index page 的部分」；`fkv_btree/src/lib.rs:137-175` `CacheLedger`、`:283`
   `with_shared_budget` 均分）。data 页两侧默认都无常驻 → data 侧预算空集成立，
   无待预算对象。

### 登记

- `doc/migration.md` §8.1 新增 4 行（页级 pin / WaitingZone / swizzle /
  data page cache），按既有「有意偏离」格式：C++ 机制 / 解决的问题 / fkv 为何
  不需要 / 何时重开。第 5 项（全局预算）已有实现，不登记。
- **COMPAT_BASELINE 登记行待协调者统一补**（eng-91 刚清理基线文件，本会话
  不动 `COMPAT_BASELINE` / `fixtures/` / `tool/fkv_ledger/`）。

### 验收（B 路径）

- 登记完整：§8.1 四行 + 重开条件逐项可查。
- 判定记录完整：本节五项均有 file:line 证据，无拍脑腘认定。
- 零代码改动：无 `fkv_cache` 新建（「有据的最小子集」前提不成立——唯一有据项
  集成点越 lane），未触碰任何 crate 源码；无需 cargo 测试。

## 复审（2026-11-05 r7）

**通过（B 路径登记，如声称完成）。**

- `doc/migration.md` §8.1「代码推断差异」（:238）表内四行登记在位：
  页级 pin / `ref_cnt`（:248）、index 页并发首访不去重 WaitingZone（:249）、
  swizzle 直取快路径（:250）、用户态 data page cache（:251）；每行含 C++ 机制
  出处（page_manager.cpp 行号）、fkv 为何不需要、重开条件，且均标注
  「已裁定/已记录：rev-41」。
- 零代码：`crate/core/` 下无 `fkv_cache` crate（亲列目录）；`ValType::SwizzlingPointer`
  仍无写入路径、`fkv_mapping` 仍仅 `is_free_slot` 防御性排除（与登记前一致）。
  core 文件 mtime 活跃（fkv_btree 03:55 等）属其它并行 lane，非本条违约面。
- 五项判定抽核：eng-67 `retain_mapped` + `(branch,term,fpid)` 校验、eng-69 系
  `CacheLedger`/`index_cache_bytes`、`fkv_config` 死旋钮（:175 解析无消费者）、
  io-21 直读分层、eng-32b 读并发窗口（≤32 在飞）等引用均为此前已复审条目，
  判定链自洽，无需重证。
- 欠账承接：DONE「COMPAT_BASELINE 登记行待协调者统一补」经核实仍未补
  （COMPAT_BASELINE 全文无 buffer pool 条目，38 份 meta.txt 因此无失同步），
  已立 `issue/new/eng-93-*.md` 跟踪（加行 + gen_fixtures 刷新 + README 路径）。
