# rev-42 RootMetaMgr LRU 与 RootMeta.first_unflushed_* 未实现（root 状态常驻、无字节预算）

## 位置
- Rust: 无 `fkv_root` crate。root 状态等价物在 `crate/engine/fkv_store/src/lib.rs:154 struct TableState`，每表一条常驻于 `table_map: HashMap<TableIdent, TableState>`（`:206`）。字段含 `manifest_size:164`、`next_expire:166`、`compression: Option<DictCompression>:167`；**无** `first_unflushed_fp_id`/`first_unflushed_seg_fp_id`、无 `ref_cnt/locked/waiting`、无 LRU/Pin/Unpin/EvictIfNeeded/pending_erase。GC 尾文件边界改按分配器高水位：`crate/engine/fkv_gc/src/lib.rs:71`（`max_fp_id >> shift`）。
- C++: `include/storage/root_meta.h:90 struct RootMeta`（`first_unflushed_fp_id_:115`、`first_unflushed_seg_fp_id_:116`、`ref_cnt_/locked_/waiting_:118`、`compression_:105`、`mapping_snapshots_/segment_mapping_snapshots_/cached_pages_`）；`:123 class RootMetaMgr`（`Entry` 双向链 + `lru_head_/lru_tail_`、`used_bytes_/capacity_bytes_`、`entries_: unordered_map`、`pending_erase_`，方法 `Pin/Unpin/UpdateBytes/EvictRootForCache/EvictIfNeeded/ReleaseMappers/ProcessPendingErase/CollectPartitions`）；`:248 struct CowRootMeta`（`old_mapping_/old_segment_mapping_/root_handle_`）。`src/storage/root_meta.cpp` 为 LRU 驱逐 + 延迟 erase 实现。

## 现状
RootMeta 的「压缩字典」维度已对齐：`fkv_store` 每表持 `DictCompression`（`compression` 字段，`fkv_btree` merge 侧 `MergeVal::batch` 用 `dict.prepare`），与 `RootMeta.compression_` 对应。但另两块未实现：
1. `RootMetaMgr` 的**按字节预算 LRU**（pin/unpin、超预算 `EvictIfNeeded`、空 entry 延迟 `pending_erase`、`ReleaseMappers`）——Rust 是 `table_map` 全量常驻，无 root/mapping 侧内存回收。
2. `RootMeta.first_unflushed_fp_id_ / first_unflushed_seg_fp_id_` 两字段及其在 file GC 中派生 in-flight 边界的用途——Rust 无对应字段。

## C++ 实现
`RootMetaMgr` 把每分区 cached `RootMeta` 挂在 LRU 上，`Handle` 构造 `Pin`、析构 `Unpin`；`used_bytes_` 超 `capacity_bytes_` 时 `EvictRootForCache`/`EvictIfNeeded` 从 LRU 尾驱逐未 pin 的 entry（释放其 mapper/快照/cached_pages）。entry 在有 reader 时变空（`root==MaxPageId && mapper==null`）只标 `pending_erase_`，下次 `Find/GetOrCreate` 无活 Handle 时才真删（`ProcessPendingErase`），避免 re-enter。`first_unflushed_*_fp_id_` 记「上一次该表 manifest 成功落盘时分配器的 `MaxFilePageId()`」（同 `max_fp_id` 语义 = 最小未分配/未落盘 id）；file GC 用 `(first_unflushed >> pages_per_file_shift)` 算 `BranchGuard::least_unflushed_file_id_`：只有 `file_id <` 该边界的文件才可能已被**某成功落盘 manifest 完整捕获**，未引用者才可删。`CowRootMeta` 是写事务的 COW 版本载体，携 `old_mapping_/old_segment_mapping_` 保证换代期间旧快照可读。

## 差异与风险
- **GC 边界语义不同源**：C++ 判「文件是否已被落盘 manifest 捕获」用的是 `first_unflushed`（持久化维度），与「是否被内存活快照引用」是两个正交条件。Rust 缺 `first_unflushed`，改由 `SnapshotRegistry` 的 retained set（内存快照维度）+ `max_fp_id` 高水位共同保证不删还需要的文件。在 append-only 下，活快照 retained 能防「正在被读」的文件被删；但 C++ 用 `first_unflushed` 额外守的是「manifest flush 落后于分配」时不删未落盘文件这一持久化约束。`fkv_gc`/manifest flush 时序不在本次审查范围，需另行核验 Rust 的 flush-vs-GC 顺序是否等价覆盖该约束——若存在 flush 未跟上而 GC 依据 max_fp_id 放行删除的窗口，即为数据丢失风险（本 issue 仅登记缺口，不臆断已发生）。
- **root/mapping 内存无预算**：每打开表的整张 `MappingTbl`（chunked 表，页数随历史累积）常驻，`IndexCache` 只约束 index 页；`RootMetaMgr` 的字节上限驱逐无对应物。分区数多或表长期打开时，mapping 侧内存单调增长、不可回收——与 C++ `capacity_bytes_` 语义的实质差距。
- 无 `CowRootMeta.old_mapping_` 显式版本链；Rust 靠 `MappingSnapshot`（Arc COW）+ `SnapshotRegistry` 弱引用达到类似的「换代期间旧读安全」，功能层面等价但实现路径不同。

## 对齐建议
优先补 `first_unflushed_fp_id`/`first_unflushed_seg_fp_id` 语义（写 manifest 成功后由分配器高水位回写）并让 file GC 以「`min(retained, 已被落盘 manifest 捕获上界)`」为准；或经论证后在 `COMPAT_BASELINE` 说明以 `SnapshotRegistry` retained-set 取代 `first_unflushed` 的等价性与其限制。若追求 parity，再实现 `RootMetaMgr` 的 pin/unpin + `EvictIfNeeded` + `pending_erase` LRU。

## 严重度
P2

## 核验记录
- 通读 `fkv_store/src/lib.rs:150-219`：`TableState` 字段确认无 `first_unflushed_*`、无 LRU/pin/evict；root 常驻于 `table_map`。
- 通读 `fkv_large`/`fkv_btree`：压缩字典 `DictCompression` 存在于 `fkv_store.compression` 与 `fkv_btree` merge，对应 C++ `RootMeta.compression_`（此维度已对齐）。
- C++ `root_meta.h` 确认 `first_unflushed_fp_id_:115`/`first_unflushed_seg_fp_id_:116`、`RootMetaMgr` LRU/`EvictIfNeeded`/`pending_erase_`/`ReleaseMappers` 与 `CowRootMeta` 真实存在。
- 全仓 grep：`first_unflushed`/`RootMetaMgr`/`EvictIfNeeded`/`pending_erase`/`CowRootMeta` 在 Rust 无任何命中。结论：压缩字典已实现且对齐；RootMetaMgr LRU 与 first_unflushed_* 未实现。GC 数据丢失风险需 fkv_gc 侧另行核验，本 issue 未臆断。

---

## 甄别结论（2026-11-05，rev-42a：部分采纳）

原意见两条主张拆分判定：**first_unflushed 部分拒绝（前提失效），RootMetaMgr 字节预算 LRU 部分采纳（修正表述后成立）**。文件已改名为 rev-42a 并移入 `issue/todo/`，仅保留字节预算差距为待开发范围。

### A. first_unflushed_* 部分：拒绝——C++ 基线自身已弃用该字段的 GC 消费

- 引用的 C++ 行号全部核实（`root_meta.h:90/105/115/116/118/123/248`），字段与注释确实存在。但意见误信了**过时注释**：C++ 提交 `44d7242`（2026-06-20，在审查基线 `82b774c` 之内）以正确性修复为由，把 `SeedActiveBranchGuardFromInMemory` 的播种源从 `RootMeta::first_unflushed_*_fp_id_` 换成**活分配器高水位** `meta.mapper_->FilePgAllocator()->MaxFilePageId() >> shift`（`src/file_gc.cpp:147-162`，注释明说原因：data-only flush 会把 `first_unflushed_seg_fp_id_` 留成 stale 0，导致死 segment 文件永远 in-flight 不可回收）。`root_meta.h:107-114`、`file_gc.cpp:281-282/1564-1565`、`tests/gc.cpp:385-393` 的注释是切换后的残留；全仓 grep 确认 `first_unflushed_*` 在基线中**只写不读**（write_task.cpp:784/824/841 写入，page_manager.cpp:187/421/465/634 透传，无任何读取方），是死写字段。
- Rust 已实现**现行** C++ 语义且已闭环：`crate/engine/fkv_task_gc/src/lib.rs` 有完整 `InflightGuard`/`BranchGuard{newest_term, least_unflushed_file_id, least_unflushed_segment_file_id}`，`Store::gc` 用内存 mapper 的 `max_fp_id() >> shift` 播种 active guard（`crate/engine/fkv_store/src/lib.rs:3325-3343`），manifest 侧由 `apply_manifest` 折叠各盘上 manifest 的 `max_fp_id/max_segment_fp_id`（`fkv_task_gc/src/lib.rs:334-339`）。与 `done/core-23-file-gc-per-branch-inflight-guard.md`、`reviewed/sto-54*` 对应，含 `tests/inflight.rs`。
- 意见担心的「flush 落后于分配而 GC 按 max_fp_id 放行删除」窗口**不存在**：分配器高水位单调 ≥ first_unflushed（分配只会前移），以高水位为删除禁区边界严格更保守。意见的「对齐建议」（补 first_unflushed 字段）反而会重新引入 C++ 已修掉的 stale-0 缺陷，不应采纳。

### B. RootMetaMgr 字节预算 LRU 部分：采纳，定性为性能/资源治理（P3），非正确性

- 差距属实：C++ `root_meta_cache_size`（默认 1GB，`kv_options.h:62`；分 shard 再除以线程数，`root_meta.cpp:206-209`）对 root meta（mapper/mapping/快照/cached_pages）做字节预算 + LRU 尾驱逐（`EvictIfNeeded`，`root_meta.cpp:410-451`）+ pin/unpin + `pending_erase_` 延迟删除。Rust 无 mapping 侧字节记账与超限驱逐。
- 但原表述「table_map 全量常驻、无内存回收」**过重**，修正为：Rust 已有 (a) `max_open_tables` 按表数的 LRU 式驱逐（`fkv_store/src/lib.rs:228`、`touch_table:521-550`，代际队列摊还 O(1)；默认 0=不限制，**默认路径确无回收**）；(b) detach/重 attach 的整表 `evict_table`（`:472-476`，之后走 reopen 重载）；(c) index 页有独立字节预算 `IndexCache`。真正缺口 = **以字节为单位的 mapping/root 内存上限**：单表 mapping 随历史无界增长、按表数驱逐管不住大表，且默认关闭。
- 开发计划（P3，资源治理 parity）：给 `TableState` 的 mapper/segment mapping 引入字节估算，`touch_table`/写入路径超 `root_meta_cache_size` 等价配置时对最久未触且无活跃快照的表整表驱逐（复用 `evict_table` + reopen-on-demand，无需 C++ 的 entry 级 pin/pending_erase——Rust 单 actor + Arc 快照生命周期天然避免 re-enter 问题）；配置项对齐 `root_meta_cache_size` 命名与 1GB 默认。
- 去重：`issue/` 全目录无 RootMetaMgr/字节预算既有条目（`eng-79` 是 fd 缓存 LRU、`io-27` 是写缓冲池、`rev-41` 是 MemCachedPage 池，均不同物）；`COMPAT_BASELINE` 未声明此偏离。`CowRootMeta.old_mapping_` 版本链一项维持原意见判断：`MappingSnapshot`（Arc COW）+ `SnapshotRegistry` 功能等价，不单独立项。

---

## DONE（2026-11-05）

采纳 B 部分（字节预算 LRU）并落地；A 部分（first_unflushed_*）维持拒绝，未改 GC。

### 实现

- `fkv_store`（`crate/engine/fkv_store/src/lib.rs`）
  - `TableState.bytes` 记账 + `table_state_bytes` 近似估算：`size_of::<TableState>()`
    + data/segment `MappingTbl` chunk（`ceil(len/CHUNK_SIZE) * CHUNK_SIZE * 8`）
    + `IndexCache::byte_len` + `DictCompression::estimated_memory_bytes`
    + `branch_metadata`（branch 名 + `file_range_li`）。
  - `Store.root_meta_cache_bytes`（0 = 不限制）+ `root_meta_used` 合计；
    `with_root_meta_cache_bytes` / `root_meta_cache_bytes` / `root_meta_bytes`
    （后者为当前状态重新汇总，诊断与测试用）。
  - `touch_table` 扩展为「表数上限 + 字节预算」共用一条 `(表, 代际号)` LRU 队列；
    超预算从 LRU 尾 `evict_table`，跳过当前表、有活快照的表
    （`SnapshotRegistry::live_num > 0`）与字典未持久化（`dirty`）的表。
  - 记账刷新点：`touch_table` 入口、`put_at` 收尾（mapping/segment 增长）、
    `install_replay_state` 收尾（open_branch 不走 touch）、`set_dictionary`；
    `evict_table` 扣减，`reset_table_li` 清零。
  - 未实现 C++ 的 entry 级 pin/pending_erase：Rust 单 actor + `&mut Store`
    独占使「正在写的表」不会被驱逐，`Arc` COW 快照自带生命周期，无需延迟 erase。
- `fkv_config`：新增 `DEFAULT_ROOT_META_CACHE_BYTES = GB`（唯一默认值来源），
  `root_meta_cache_size` 默认值改引用它（仍 1 GiB）。
- `fkv_shard`：`ShardConfig.root_meta_cache_bytes` 新字段（测试构造统一补 0）。
  并行 lane 的 per-partition Store 重构（`partition.rs`）进一步把执行点上移到
  `PartitionStoreMap`：新分区创建后按 `Slot.touch`（读 / 写 / 打开都更新）求
  近似 LRU，跳过 `Rc::strong_count > 1`（仍被读任务持有）的分区后逐分区
  `evict_table`；单分区 `Store` 显式 `with_root_meta_cache_bytes(0)`，合计占用
  复用本次新增的 `Store::root_meta_bytes()`。`config.root_meta_cache_bytes`
  仍经 `PartitionStoreMap::new(...)` 接线，语义一致（每 Store 一个表）。
- `fkv_runtime`：`KvOption.root_meta_cache_size → ShardConfig` 投影，按
  `num_threads` 均分（对齐 C++ `root_meta.cpp` 的
  `capacity_bytes_ = root_meta_cache_size / num_threads`），0 保留不限制。

### 测试

新增 `crate/engine/fkv_store/tests/root_meta_budget.rs`（4 条）：
1. `root_meta_budget_evicts_lru_table_and_reloads`：超预算按 LRU 驱逐、reopen 重载；
2. `root_meta_budget_keeps_tables_with_live_snapshot`：活快照表不驱逐，drop 后下一轮回收；
3. `root_meta_bytes_include_index_cache_and_release_on_evict`：index cache 计入预算且随整表驱逐释放；
4. `root_meta_budget_default_is_unlimited`：默认 0 = 不限制，行为不变。

新增 `crate/engine/fkv_runtime/tests/root_meta_budget.rs`：
`root_meta_cache_size_reaches_store` 用 1 字节预算验证 KvOption→Runtime→Shard→Store
接线（写第二张表后第一张表的 index 缓存占用下降，读取走 reopen 数据完整）。

验证命令（targeted）：

```
cargo nextest run -p fkv_store                                  # 139 passed
cargo nextest run -p fkv_store --test root_meta_budget          # 4 passed
cargo nextest run -p fkv_config --test config                   # 23 passed
cargo nextest run -p fkv_shard --test index_cache_metrics --test lifecycle \
  --test backend --test read_batch                              # 11 passed
cargo nextest run -p fkv_runtime --test options                 # 5 passed
cargo nextest run -p fkv_runtime --test root_meta_budget        # 1 passed
cargo clippy -p fkv_store -p fkv_shard -p fkv_runtime -p fkv_config \
  --all-targets -- -D warnings                                  # 干净
```

### 27b 快照引用计数：无需新增，最小闭环已满足

- 读请求入口 `Store::snapshot` 取 `MappingSnapshot`（`Rc<SnapshotCore>` 强引用）
  并登记 `(表, Weak)`；`live_num(tbl)` 即「仍被读请求引用」。
- 预算驱逐只在 `&mut Store` 内发生，与在飞读任务互斥（shard 先
  `finish_read_li` 才拿独占）；即使绕过 shard 直接持有快照，`live_num > 0`
  也会跳过该表，快照 drop 后下一轮触摸即可回收。
- 因此不需要 per-table pin 计数 / `CowRootMeta` 版本链；只有将来允许
  「读与 `&mut Store` 并发」时，才需要把 `live_num` 换成原子 pin 计数。

### 风险 / 限制

- 纯读工作集不触发预算复查：touch 只在写 / reopen / open_empty 发生，
  index 缓存的读增长由下一次触摸补记；读放大由 `index_cache_total_bytes`
  独立兜底。与 C++ Find 即 touch 存在差异，属资源治理精度而非正确性。
- 单表大于预算时接受超限（当前表不驱逐自己），否则会每次写都自行卸载；
  多表时仍会驱逐其它 LRU 表。
- 驱逐复用既有 `evict_table`，会一并清 IO 层（fd / cloud 本地 cache），
  比 C++ `ReleaseMappers` 更强；随后按需 reopen。
- `Store::new` 默认不限（0），产品默认 1 GiB 经 runtime 注入；直接使用
  `Store` 的测试 / 嵌入方需显式开启。
- 验证时点：fkv_shard 并行 lane 正在做 per-partition Store 重构（`partition.rs`
  与 `ShardState` 改写，会改动本次的 shard 接线形态）。上述 shard / runtime
  测试是在该重构落地前跑的全绿结果；最终 shard / runtime `--all-targets`
  复跑需待并行 lane 编译恢复后补验。`fkv_store` / `fkv_config` 的测试与
  clippy 不受影响，已复跑。
- 未 `git commit`（按并行 lane 约定）。

## 复审（2026-11-05 r7）

**通过。** 重点核对项（字节预算真接线：config 旋钮 → 运行时消费；上限测试）逐行亲读：

- 记账：`TableState.bytes` :193-197 + `table_state_bytes` :226-253
  （`size_of::<TableState>` + data/segment mapping chunk `ceil(len/CHUNK_SIZE)
  * CHUNK_SIZE * 8` + `DictCompression::estimated_memory_bytes` + `IndexCache::
  byte_len` + branch 元数据）；`refresh_table_bytes` :663-673 更新点齐备——
  `touch_table` 入口 :628、`set_dictionary` :762、`put_at` 收尾 :1556（随后
  `evict_over_root_meta`）、`install_replay_state` 收尾 :2114（open_branch 不走
  touch）、`evict_table` 扣减 :575、`reset_table_li` 清零 :1850。
- 驱逐：`touch_table` :624-660 表数上限与字节预算共用 `(表, 代际号)` LRU 队列；
  `evict_over_root_meta` :681-700 超限从 LRU 尾整表驱逐，跳过当前表与
  `root_meta_evictable` :703-711 不通过的表（`snapshots.live_num(tbl) == 0`
  且字典非 `dirty`）；单轮有界扫描不死循环（`scanned` :685-686）。
- **接线全链路**：`fkv_config` :57 `DEFAULT_ROOT_META_CACHE_BYTES = GB`（唯一
  默认值来源）→ :183 `root_meta_cache_size` INI 旋钮（parse_size）→
  `fkv_runtime` :1190-1200 `KvOption.root_meta_cache_size` 按 `num_threads`
  均分（对齐 C++ `root_meta.cpp` `capacity/threads`，0 保留不限）→
  `fkv_shard` `ShardConfig.root_meta_cache_bytes` :149 → :571
  `PartitionStoreMap::new(..., config.root_meta_cache_bytes)` →
  `enforce_root_meta_budget`（partition.rs :325-363，按 `Slot.touch` 近似 LRU、
  跳过 `Rc::strong_count > 1` 仍被读任务持有的分区）；单分区 Store
  `with_root_meta_cache_bytes(0)`（partition.rs :313）。`Store` 侧入口
  `with_root_meta_cache_bytes` / `root_meta_cache_bytes` / `root_meta_bytes`
  :555-568。
- 上限测试亲读断言：fkv_store/tests/root_meta_budget.rs 4 条——1 字节预算 LRU
  驱逐 + reopen 数据完整（:27-55，断言 mapper 卸载 / 重载值）、活快照表不驱逐
  且 drop 后下一轮回收（:59-80）、index cache 计入预算且随整表驱逐释放
  （:84-118，`root_meta_bytes >= mapping + cache`）、默认 0 不限（:122-133）；
  fkv_runtime/tests/root_meta_budget.rs `root_meta_cache_size_reaches_store`
  （:56-105）经 `KvOption{root_meta_cache_size: 1}` 全链路验证写第二张表后
  `index_cache_bytes` 下降 + 被驱逐表读取 reopen 数据完整——配置旋钮到运行时
  消费为真，非仅字段存在。
- DONE 自认的限制（纯读工作集不触发复查、单表超预算接受超限、shard 并行 lane
  重构后需补跑 --all-targets）与源码一致，非失实。

