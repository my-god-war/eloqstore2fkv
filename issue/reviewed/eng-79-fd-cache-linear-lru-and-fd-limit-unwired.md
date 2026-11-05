# eng-79 FdCache 名为 LRU 实为“每次插入全表扫两遍”；`fd_limit` 配置从未接线，有效上限固定 256（C++ 默认 10000）

## 位置

- `crate/io/fkv_io_compio/src/lib.rs:52`：`const FD_CACHE_LIMIT: usize = 256;`
  作为 `FdCache::default()` 的 `limit`（`:128-138`）。
- 同文件 `:175-191`（`insert`）：`slot.insert(...)` 后若 `len() > limit` 调 `evict_oldest`。
- 同文件 `:200-229`（`evict_oldest`）：对 `read` 和 `write` 两个 `HashMap` 各做一次
  `iter().min_by_key(|(_, c)| c.used)`（两次全表扫描），再各做一次 `get` 取 `used`
  （又是两次查找），最后 remove 一个。
- 同文件 `:551-555`（`with_fd_cache_limit`）：唯一调用方是
  `crate/io/fkv_io_compio/tests/hardening.rs:260`；`Shard` / `Runtime` / `KvStore` 均未调用。
- `crate/foundation/fkv_config/src/lib.rs:27,98,268`：`fd_limit` 默认 10_000、可从 INI 解析，
  但全仓无消费者。
- C++：`include/kv_options.h:70` `fd_limit = 10000`；
  `src/async_io_manager.cpp:2671-2690` `EvictFD()` 从侵入式双向链表尾取一个
  （`lru_fd_tail_.prev_`），O(1) 关闭一个，循环到 `lru_fd_count_ <= fd_limit_`；
  链表头尾在 `:173-174` 初始化。

## 现状

1. **有效上限与配置不一致**：C++ 默认 10_000，Rust 实际 256，且 `fd_limit` 写了也不生效。
   多表/多文件负载下 fd 反复 close/open；`fd_open_num` 指标会直接暴露这一点。
2. **淘汰复杂度**：当前 `limit=256` 时每次超限插入扫 256 条读 + 256 条写；
   一旦按配置接到 10_000 且不换数据结构，warmup 阶段是 O(n²)（10k 次 open ≈ 5e7 次比较）。
3. `FdCache.clock` 单调递增 + `used` 字段本身没问题，但“找最小值”用线性扫描实现，
   等于把 LRU 退化成“每插入一次做一次全表 min”。

## C++ 实现

C++ `IouringMgr` 用侵入式 `LruFD` 双向链表：`OpenFD` 命中即 move-to-front，
`EvictFD` 从 tail 取，O(1)；`fd_limit` 在启动时按 shard 均分
（`src/eloq_store.cpp:841-842` `shard_fd_limit = store_fd_limit / num_threads`）。
`CloseFile` 只关闭 ref_count=0 的尾部 fd。

## 建议

1. 用索引式侵入式 LRU 替换 `HashMap + min_by_key`（避免 unsafe）：
   ```rust
   struct LruEntry { path: PathBuf, file: File, prev: Option<usize>, next: Option<usize> }
   struct FdCache { nodes: Vec<LruEntry>, free_slots: Vec<usize>,
                    map: HashMap<PathBuf, usize>, head: Option<usize>, tail: Option<usize> }
   ```
   `get` 命中 → `unlink + push_front`；`insert` 超限 → `pop_tail`；全部 O(1)。
   读写两种 Access 可以各一份 LRU，或把 `access` 放进 key。
2. 接线 `fd_limit`：`Shard::start` 把 `KvOption.fd_limit / num_threads`（至少 1）
   传给 `CompioStore::with_fd_cache_limit`；`KvStore` facade 同样处理。若暂不接线，
   至少把 `FD_CACHE_LIMIT` 改到与配置默认一致并在文档中说明“固定 256”这一偏离。
3. 先加压力测试：打开 N（>limit）个不同路径的页，断言 `fd_cache_len() <= limit`、
   `fd_open_num()` 不随重复访问增长；再接 10k 上限。
4. `evict_oldest` 的 `map_or(u64::MAX, ...)` 在刚 min 出的 key 上做二次查找，属于可删除的防御；
   索引化后自然消失。

## 严重度

P2（当前有效上限 256 已影响多表 fd 复用；接线 10k 前必须先换数据结构，否则性能反向）

## 核验记录（2026-11-04）

`FD_CACHE_LIMIT`、`insert`、`evict_oldest`、`with_fd_cache_limit` 实读；
全仓 grep `with_fd_cache_limit` 仅测试一处、`fd_limit` 除 config 外零消费。
C++ `fd_limit` 默认、`EvictFD` 链表实现、按 shard 均分引用自
`include/kv_options.h:70`、`src/async_io_manager.cpp:2671-2690`、`src/eloq_store.cpp:841-842` 实读。

## 审查结论（2026-11-04）

核验结果：**合理**，位置基本准确（`insert` / `evict_oldest` 现位于 `:216-282`）。

- `FD_CACHE_LIMIT = 256`（`:52`）仍是 `FdCache::default()` 的 `limit`（`:128-138`）。
- `insert`（`:216-241`）= `HashMap::insert` 后 `if len() > limit { evict_oldest() }`，
  未做“同 key 已在缓存”的显式处理（覆盖同 key 不会增长 len）。
- `evict_oldest`（`:249-280`）对 read / write 各做一次 `iter().min_by_key(used)`，
  再对选中的 key 各 `get` 一次取 `used`，最后 remove 一个：O(n) 两次全表扫描。
- `with_fd_cache_limit`（`:723-726`）唯一调用方是 `tests/hardening.rs:260`；
  `fkv_shard` / `fkv_runtime` / `fkv_sdk` 均未接线。
- `fd_limit` 仅出现在 `fkv_config`（`:30,101,302`），全仓零消费属实。
- C++ 侧 `fd_limit` 默认、`EvictFD` 双向链表 O(1)、按 shard 均分三个引用属实。
- 建议 4 的二次查找确属可删防御，索引化后自然消失。

结论：拆成两个 todo 分步落地，先换数据结构并默认值对齐 C++ 10_000，再接线配置并补压力测试。

## TODO（2026-11-04）

- `issue/todo/eng-79a-fd-cache-o1-lru.md`：索引式侵入链表 LRU，O(1) 命中/插拔/淘汰。
- `issue/todo/eng-79b-fd-limit-wiring-and-test.md`：`fd_limit` 从 KvOption 经
  Runtime / Shard / SDK 接到 `CompioStore`，补压力测试。

## DONE（2026-11-04）

两个子任务均已落地：

- `eng-79a`：索引式侵入链表 LRU，O(1) 命中 / 插入 / 淘汰；默认上限 10_000。
- `eng-79b`：`fd_limit` 从 `KvOption` 经 `Runtime` / `Shard` / `SDK` 接到
  `CompioStore`；新增压力测试。

验证：`cargo nextest run -p fkv_io_compio -p fkv_config -p fkv_shard -p fkv_runtime`
84 passed, 1 skipped；clippy 0 warning。

## 复审（2026-11-05 r6）

通过（拆分总纲，两个子项 79a/79b 均已独立复核通过，见各自 r6 小节）。

- O(1) LRU（`FdNode`/`FdCache` 槽位数组 + 双向链 + 双索引）与
  `FD_CACHE_LIMIT=10_000` 见 79a 小节；`fd_limit` 全链接线
  （config → runtime 均分 → ShardConfig/backend → CompioStore/SDK）见 79b 小节。
