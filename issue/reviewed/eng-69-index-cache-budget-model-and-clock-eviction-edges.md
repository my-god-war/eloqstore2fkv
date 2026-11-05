# eng-69 缓存预算模型：每表独立 64 MiB 无聚合上限（C++ shard 级共享单池 + 写缓冲预留 5%）；Clock 逐出边界两处缺陷

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:109-141`（`index_cache_map: HashMap<TableIdent, RefCell<IndexCache>>` 逐表独立，无聚合字节账目）；`crate/core/fkv_btree/src/lib.rs:126`（每表 64 MiB）；`:192-193`（Clock 逐出：`budget = order.len()*2`、`order.len() > 1` 才逐出）
- C++: `src/storage/page_manager.cpp:29-44`（`EffectiveBufferPoolLimitBytes = buffer_pool_size − buffer_pool_size*write_buffer_ratio`）；`include/kv_options.h:52,185`（`buffer_pool_size=128MB`、`write_buffer_ratio=0.05`）；`page_manager.cpp:145-150`（index 与 data 页**共池**计账）；`:776-798`（Evict：LRU 尾严格推进，pinned 跳过，到头才 false）；`:95-124`（AllocPage 满则逐、逐不动返回 nullptr → OutOfMem 回滚重试，容量硬约束）

## 现状（两个子问题）
1. 预算模型：Rust 缓存总量 = 64 MiB × 打开表数 + 每表 mapper/字典，无全局约束无背压；C++ 单池内 index/data/写缓冲三方分账、超限即逐。
2. Clock 边界：全部命中（referenced 置位）时 budget 耗尽即退出，`bytes > capacity` 可持续多轮；`order.len() > 1` 使唯一超大条目永不被逐；`byte_len` 只计 `key.len() + size_of<(Vec<u8>,PageId)>()`（`:116-122`），不计堆分配粒度/头部 → 实际占用系统性低估，预算进一步失真。

## 差异与风险
多表场景 RSS 随表数线性放大且无背压；缓存可稳定超预算。

## 对齐建议
1. 若保留每表独立预算（done 46 的既定方向），补 shard 级聚合上限或按表数缩放默认值，并记录该设计决策与 C++ 分账差异；
2. `order.len() > 1` 改为允许逐出最后一项（单条超预算直接放弃缓存该条）；
3. budget 改「至多两整轮」精确语义；`byte_len` 计入分配粒度。

## 严重度
P2（多表内存行为）/ P3（Clock 边界）

## 核验记录（2026-11-04）
Rust 预算与 Clock 行号实读确认；C++ 分账/硬预算语义引用自子代理摘录。与 done 46（每表预算引入）相邻但角度不同（聚合上限与逐出边界），分流时注明。

## 审查结论（2026-11-04）

判定：合理，拆分落地。描述的三处缺陷实读仍存在：

1. `order.len() > 1` 使唯一超大条目永不被逐出；
2. Clock 扫描预算耗尽后 `bytes > capacity` 可持续多轮（全部 referenced 时）；
3. `byte_len` 只计 `page.capacity()`，不含堆分配粒度与块头。

预算模型现状核实：`Store` 单线程持有全部 `table_map`，天然是 shard 级共享句柄，
因此不必跨线程共享池，只做「Store 级聚合上限 + 按缓存实例均分」这一可行子集。
C++ index/data 共池、写缓冲 5% 预留不照搬（fkv 无统一 buffer pool，data 页不缓存）。

拆分：
- `eng-69a-clock-eviction-boundaries.md`（已实现 → done）
- `eng-69b-index-cache-aggregate-budget.md`（已实现 → done）
- `eng-69c-index-cache-total-bytes-knob.md`（遗留：配置键贯通 + C++ 分账差异文档）

默认聚合上限跟随每表预算（初始 `DEFAULT_INDEX_CACHE_BYTES`，64 MiB）：单表行为
不变，多表合计不再随表数线性放大。`with_index_cache_total_bytes(0)` 关闭聚合约束。

与 done 46 / eng-66b 的关系：每表 64 MiB 旋钮保留，新增的是跨表总量约束，
不改变 `CacheStat.capacity` 的「每表配置」语义。

## DONE（2026-11-04）

`eng-69a` / `eng-69b` / `eng-69c` 均已完成：Clock 两轮逐出边界、CacheLedger 共享账本
与 shard 级聚合预算、`index_cache_total_bytes` 旋钮接线。父 issue 归档。

## 复审（2026-11-05 r6）

通过（父条目）。eng-69a / 69b / 69c 均已按当前源码复核，见各自 r6 复审小节。
