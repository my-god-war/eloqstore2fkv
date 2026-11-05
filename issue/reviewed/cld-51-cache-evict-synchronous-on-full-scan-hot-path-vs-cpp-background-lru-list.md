# cld-51 云 cache 淘汰在读写热路径同步全表扫描选 victim，C++ 后台异步 + 侵入式 LRU 链表 O(1)

## 位置
- Rust: `crate/engine/fkv_cloud/src/lib.rs:411-454`（`evict_candidate`：对 `file_map` 与 `segment_map` 各做一次 `iter().filter(!dirty).min_by_key(last_used)` 全表扫描）；`:458-476`（`evict_to_budget`：`while let Some(victim) = evict_candidate()` 逐个 `local.remove_file` + `forget`）；热路径调用点 `:525,617,651,666,707,726,748,810,829,841,1035,1044,1051`（`write_page`/`write_page_li`/`write_segment`/`read_page` 命中与未命中、`read_segment` 命中路径都调 `evict_to_budget`）
- C++: `eloqstore/src/async_io_manager.cpp:4228-4258`（`TrimRestoredCacheUsage` 用侵入式链表 `lru_file_tail_.prev_` O(1) 取尾）、`:7089-7122`（后台 `FileCleaner` 任务遍历 `lru_file_` 链表批量 unlink）、`:6160-6171`（`HasEvictableFile` 判 `lru_file_tail_.prev_ != head`）、`:3589-3590`（`lru_file_head_/tail_` 侵入式 LRU）

## 现状
Rust 淘汰是**同步、内联、O(N)**：
1. `evict_candidate`（`:411-454`）在 `file_map`+`segment_map` 上各做一次 `min_by_key(last_used)` 全表线性扫描找最久未用且非 dirty 的文件。
2. `evict_to_budget`（`:458-476`）循环调用它，每次淘汰一个文件就重新全表扫描再找下一个，直到 `resident_bytes <= budget`。
3. 该逻辑被挂在**每个写页/读页（含命中）/写段/读段**的尾部（`:748,810,829,841` 等），即使 cache 没超预算，`evict_candidate` 也要先 `borrow` 判断——超预算时更是每次操作触发多轮 O(N) 扫描 + 同步 `remove_file` await。

## C++ 实现
C++ 维护**侵入式 LRU 双向链表** `lru_file_head_`/`lru_file_tail_`（`async_io_manager.cpp:3589-3590`）：
- 取 victim 是 O(1) 读 `lru_file_tail_.prev_`（`:4230`），淘汰后 `victim->Deque()` 摘链；
- 淘汰在**后台 `FileCleaner` 协程**里做（`:7089-7122`），批量 unlink、与前台读写解耦，不在写/读热路径同步删文件；
- `dirty`（未上传）文件不进 closed_files_ LRU，天然不被淘汰（等价 Rust 的 `!dirty` 过滤）。

## 差异与风险
- **热路径 O(N) 扫描（P2）**：每次写页/读页命中都要 `min_by_key` 扫全 resident 表。resident 文件数 N 大（cloud 模式 cache 装几千文件）时，每次点读/写页叠加 O(N) 选择开销，直接抬高 p99，违背 todo.md §1.1“点读恰好 1 次磁盘 IO”“同硬件 p99 同量级”的目标。
- **同步淘汰阻塞热路径**：`remove_file` 是 `await`，内联在 write_page/read_page 里会让前台请求等待本地文件删除；C++ 把删除丢到后台 FileCleaner。
- **每轮重扫**：`while` 循环每淘汰一个就重扫全表，M 个 victim → M×O(N)。
- 与 C++ 的差异：C++ 是 O(1) 取尾 + 后台批量，Rust 是 O(N) 取尾 + 前台内联。

## 对齐建议
- 用侵入式 LRU 链表或 `lru`/带 generation 的堆替换 `min_by_key` 全表扫描，victim 选取降到 O(1)/O(log N)，对齐 `lru_file_tail_`。
- 把淘汰从 write/read 热路径移到后台任务（或 `tokio::spawn`/后台协程），`evict_to_budget` 只置“超预算”标志 + 唤醒后台，对齐 FileCleaner。
- `evict_to_budget` 在 `resident_bytes <= budget` 时早退（`:413` 已有判断，但调用点 `:810` 命中路径仍每次调，需确认早退成本）。

## 性能优化建议
- victim O(N)→O(1) + 后台淘汰：消除读写热路径的 cache 管理开销，resident 规模越大收益越大，是云模式 p99 的直接优化点。

## 严重度
P2（读写热路径 cache 淘汰 O(N) 同步扫描，抬高 p99）

## 审查结论（2026-11-04）

**部分采纳（P2），转 todo**。

- 事实修正：`evict_candidate` 在 `resident_bytes <= budget` 时直接返回，
  未超预算的热路径只有 borrow + 比较，不做全表扫描。
- 成立部分：超预算时 `evict_to_budget` 每淘汰一个 victim 重新全表
  `min_by_key`，M 个 victim = M×O(N)；且 `remove_file` 同步内联在读写路径。
- 修复方向：用带 generation 的最小堆 / 侵入式 LRU 做到 O(log N)，并把
  删除移到 CloudStore 后台维护任务。
- 依赖：`cld-47` prewarm 后台化后可复用同一后台执行器。

拆分执行：
- `cld-51a-cache-evict-intrusive-lru-o1.md`：全局侵入式 LRU，victim O(1)。
- `cld-51b-cache-evict-off-hot-path-background.md`：删除移出热路径，
  阻塞于执行器注入（`12-runtime-modes-api` / `cld-47`）。

## 复审（2026-11-04 10:05，父条目收口）

本父条目的三个方向逐条结清，无遗留唯一工作，故移 done；真正的待开发项是
`issue/todo/cld-51b-...md`（在 todo 清单里，不在本文件内）。

- **O(N) victim 选取 → O(1)**：已落地（`cld-51a`，issue/done/）。现
  `evict_candidate`（`crate/engine/fkv_cloud/src/lib.rs:884-896`）在
  `cache_budget == 0 || resident_bytes <= cache_budget` 时直接返回，否则读
  `state.lru_head`（全局侵入式 LRU 头）O(1) 取 victim，`oldest_clean_data` /
  `oldest_clean_segment` 两个全表扫描函数已删除。
- **未超预算热路径成本**（父条目「需确认早退成本」）：确认无需改动。未超预算时
  `evict_to_budget` 只做 `RefCell::borrow` + 两次整数比较即返回，不做遍历、
  不取 victim、不 await 删除；`evict_to_budget` 的 14 个调用点（
  `:990,1086,1120,1135,1176,1195,1225,1291,1310,1322,1560,1577,1586,1593`）
  在该路径上都是常数开销。
- **删除移出读写热路径**：未实现，`evict_to_budget`
  （`crate/engine/fkv_cloud/src/lib.rs:923-941`）仍内联 `local.remove_file` /
  `remove_segment_file` 的 `await`。阻塞证据见
  `issue/todo/cld-51b-cache-evict-off-hot-path-background.md` 的「阻塞复核」：
  fkv_cloud 无运行时句柄，注入点在 lane 外，且半成品会破坏磁盘预算语义。

后续动作只发生在 `cld-51b`；本文件不需要再回到 todo。

## 复审（2026-11-05 r6）

**通过（父条目收口结论维持）。** 抽验当前源码：`fkv_cloud/src/lib.rs` O(1) LRU（`lru_head`/`lru_unlink`/`lru_move_tail`，:280 `CacheState.lru_head`）仍在；「删除移出热路径」已由 cld-51b 于 11-05 落地（`trim_cache`:1045 + `drain_evictions_if_due`:1227 阈值批量），比本父条目记录的「阻塞于执行器注入」更进一步，方向一致，无回归。无剩余唯一工作。
