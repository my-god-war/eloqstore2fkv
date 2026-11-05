# r6 复审报告：core/format 基础层

日期：2026-11-05。Lane crate：`crate/core/{fkv_btree,fkv_buf,fkv_large,fkv_mapping}`、
`crate/format/{fkv_page,fkv_manifest,fkv_compress}`、`crate/foundation/{fkv_codec,fkv_types}`。
约束遵守：只读源码，未改 crate/，未运行构建；仅写 issue/new/、issue/reviewed/ 并从 done/ mv。

## 任务 A：done 复核 —— 42 条，42 通过，0 未过

逐条按 md 声称的实现要点/测试到当前源码核对（关键断言抽读，非仅文件名），全部核实后在
原 md 末尾追加「## 复审（2026-11-05 r6）」（含 file:line 证据），再 mv 至 issue/reviewed/。

| 组 | 条目 |
|---|---|
| 格式保真 | fmt-16a、fmt-17、fmt-17b、fmt-18、fmt-18a、fmt-18b、fmt-19、fmt-20、fmt-20a、fmt-21 |
| mapping/分配器 | core-11、core-12、core-22（父）、core-22a、core-26、core-27、core-28、core-28a、core-29 |
| buf/快照 | core-18、core-18a、27a |
| IndexCache（fkv_btree） | eng-69a、eng-69b、eng-69c、eng-66a、eng-66b、eng-70a、eng-70b |
| manifest | eng-76a、eng-76b、eng-76c |
| 结构/死代码 | eng-71、eng-78、eng-80、eng-81、eng-83a、eng-87 |
| codec/写路径边界 | eng-85、rev-01、rev-01a、rev-02 |

代表证据（详见各 md 复审小节）：

- fmt-16a 金标：`fkv_page/tests/format_semantics.rs:224/:283`，逐字节 `assert_eq` + padding 全 0。
- core-11：`fkv_mapping/src/lib.rs:814-820`（先 allocate 再 free）、`fkv_store/src/lib.rs:72-76`
  （STORE_ALLOC_MODE 编译期断言）、`tests/mapping.rs:155-165`。
- core-12：`fkv_mapping/src/lib.rs:311/:367/:400/:407/:417/:893-905` + store 接线
  `fkv_store/src/lib.rs:1787/:2991/:3247`；4 个 mapping 测试 + 2 个 gc_store 测试在位。
- 27a：`fkv_mapping/src/lib.rs:955/:1024/:1046/:1084-1089`、`fkv_store/tests/gc_snapshot.rs:49/:129`
  （快照独占文件 GC 不删 / drop 后回收）。
- eng-76a/c：`fkv_manifest/src/lib.rs:218/:255/:316` + `tests/manifest.rs:70-97` 跨 3 chunk
  切片/迭代器逐字节一致。

轻微账实差（已在对应 md 复审小节注明，不构成不通过）：

1. eng-76b DONE 中「`value_li` 保留」已过时——后续清理连该便捷方法一并删除，测试统一
   `iter_values().collect()`，无行为影响。
2. eng-70a 交付的 `raw_page_seek_matches_linear` 已被 eng-87 随 `seek_index` 删除（新旧对比
   失去一方），seek 正确性现由 `fkv_btree/tests/seek.rs:23` + fkv_page 自身 seek 测试覆盖。
3. core-12/27 等 store 侧行号相对 DONE 小节 ±3 漂移（fkv_store 为并行热点文件），语义全部在位。

跳过（按内容归属其他 lane，未复核）：eng-53、eng-62a、eng-63a/63b（io/read）、eng-64/65a/65b/86/88
（read/scan）、eng-68a/68b/73a（store）、eng-74a/74b/74c、eng-79a/79b、io-*（io）、eng-59/60b/60d/61、
eng-32a/32b/33b/34c/39*、20-write-batching、27b、12*、32a/32b、33a、cfg-01、rev-03a/03b、cld-*（含
core-22b、cld-56）、core-23/25、sto-*、perf-*、eng-77/82、01-cpp-oracle（工具链）。
其中 fmt-17b/eng-70b/rev-01a 等落点虽在 task_write/task_read/store 热点文件，但契约两端在
本 lane crate，已按只读方式核对。

## 任务 B：新问题 —— 0 条

六轮扫描后按「宁缺毋滥」执行，探测面与结论：

1. **fkv_compress vs `src/compression.cpp`**：SKIP=100 / STANDALONE=10MiB / 字典采样
   （100×dict、min 5 样本）/ prepare 分派顺序均对齐；「前缀+压缩体 vs 仅压缩体」的保留判定
   差异已在代码注释与 doc/migration.md §8.1 记录（有意偏离，勿立项）。
2. **AppendAllocator 统计 vs `page_mapper.cpp:655-696` + `replayer.cpp:358-390`**：
   SpaceSize 公式、UpdateStat snap-up（Rust 拆为 update_stat + skip_to_next_file，调用点等价）、
   file_stat 空洞计数（位图 [min,max_live] == C++ 相邻占用文件 gap 计数）逐行一致；core-12 DONE
   中「反推语义待 oracle 确认」一项本轮已用本地 C++ 源确认对齐。
3. **overflow 页布局 vs `data_page.cpp:586-682`**：header 11B + value + 尾部指针数组 +
   num_pointers 末字节、capacity(end) 扣减、first_pointer_count 公式一致。
4. **IndexCache 代际失效**：get/insert generation 门 + retag + `generation_gates_hits_and_retag_refreshes`
   在位，remap/free 路径 bump_generation。
5. **chunk 池不变式**：`take_chunk` clear 后 len=0，`Chunk::filled/clone` 均全量填充，无 stale
   值泄漏；pool 上限 256 块/线程。
6. **page_size > 64KiB**：C++ 字段 uint16_t + `kv_options.cpp:334-339` 静默截断，Rust config 放行但
   `fkv_page/header.rs:96` builder 拒绝——Rust 更安全，且 C++ 行为本身是截断而非干净报错，不构成
   值得对齐的分歧，不立项。

## 遗留风险（均有 issue 跟踪，非缺口）

- 27a 残余：跨 Store 实例/branch 的读者快照互不可见、drop_table 不检查活跃快照、IndexCache
  未随快照固定——归 27b/后续。
- core-11 残余：Pooled 快照代际链仍未实现，靠 `STORE_ALLOC_MODE` 编译期守卫封死误用路径。
- fmt-16a 未覆盖面（codec/compress/manifest 字节级 oracle、_cpp crate FFI 形态）仍归
  `01-cpp-oracle-and-fixtures`。
- 压缩保留判定、MappingTbl 收缩回填（core-29）为已记录有意偏离，后续对齐勿误改。

## 最终判定

**完善**。零新立项；lane 内 done 条目 42/42 复核通过并移入 reviewed/；本轮六个探测面
（compress、allocator 统计、overflow 布局、cache 代际、chunk 池、页大小边界）未发现
未被现有 issue 覆盖的差异。
