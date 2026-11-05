# rev-40 KvOptions::partition_filter 未实现：多 data-store-shard 托管路由缺失（已登记决策，补跟踪）

## 处置（2026-11-05 甄别）

**非拒绝，合并进 sto-61b。** 本条事实全部属实（C++ 消费点行号已逐处复核），但
`partition_filter` 未实现是 fkv_config:11-12 已登记的决策（随 sto-61 的 `DsShardId`
路由落地），且 `issue/todo/sto-61b-multi-ds-shard-hosting-and-request-routing.md`
的方案（per-request `ds_shard`、attach 校验、路由、detach fanout、`KvOption` 补
字段）已实质覆盖其落地场景，独立保留即重复跟踪。本条增量价值——C++ 消费点全量
清单（eloq_store.cpp 7 处 + `EvictDsShard` + prewarm）——已作为
「## rev-40 合并（2026-11-05 r6）」小节并入 sto-61b 的验收清单。

## 位置
- Rust: `crate/foundation/fkv_config/src/lib.rs:11-12`（未迁移字段登记）；全仓 `grep partition_filter` 仅命中该注释；`crate/engine/fkv_runtime/src/lib.rs:1279-1282`（`Runtime::shard` 固定 `partition_id % num_threads`）；`crate/engine/fkv_task_read`→ `fkv_runtime` archive crond 的目标枚举（`:302-330`）无任何过滤钩子
- C++: `include/kv_options.h`（`partition_filter` 定义）、`src/eloq_store.cpp:100-101, 1541-1550, 1728-1729, 1780-1781, 1958-1959, 2304-2305, 2357-2358`（attach 扇出 / global archive create+delete / global branch / global reopen 逐处过滤）、`src/storage/shard.cpp:800-803`（`EvictDsShard` 按过滤器决定 detach 驱逐哪些分区）、`src/tasks/prewarm_task.cpp:515-516`（预热目标过滤）

## 现状
fkv 没有 `partition_filter` 的任何实现：路由唯一由 `TableIdent::shard_index =
partition_id % num_shards` 决定；全局操作（archive / create_branch / drop / reopen）对
`enumerate_targets()` 的全部扇出，crond 只按 `attached[shard]` 布尔过滤；detach 驱逐
「本 shard 打开过的全部表」。这是有记录的决策（fkv_config 注释：属「一个 Store 托管多
个 data store shard」，随 sto-61 的 `DsShardId` 路由一并落地），当前 runtime 确实没有
per-request ds_shard 概念，worker shard 与 data store shard 一比一。

## C++ 实现
C++ 的 `partition_filter(ds_shard_id, tbl_id)` 是公共契约：把「哪个 data store shard
托管哪个分区」从固定取模泛化为回调，attach / EvictDsShard / 全部 Global* 扇出 /
prewarm 六个消费点全部经过它（`eloq_store.h:1003/1032` 明确 detach 与 filter 的语义
约定）。默认（不设 filter）退化为「全部归本 shard」，与 fkv 的取模路由**不等价**——
C++ 默认路由是 `ds_shard_id % shards_.size()`（`eloq_store.cpp:1020`，请求携带
ds_shard=分区号），这一点两侧一致；分歧只在自定义映射能力。

## 差异与风险
- 无法把非连续/加权分区映射到 shard 集合（elastic 场景需要），也无法实现「一个 worker
  shard 只服务部分分区」的部署形态。
- 影响面与 C++ 语义绑定：将来补 `DsShardId` 时，archive crond、prewarm、detach 驱逐三
  处都必须同时接 filter，缺一个就扇出越权（对不属于本 shard 的分区发 CreateArchive /
  Drop）。现在登记可让 sto-61 落地时一起接。
- 不阻塞当前 Local/Cloud 单形态功能：默认路由行为与 C++ 一致，已核对。

## 对齐建议
sto-61 落地 `DsShardId` 时同步引入 `KvOptions::partition_filter:
Option<Arc<dyn Fn(u32, &TableIdent) -> bool + Send + Sync>>`，接入点清单照抄 C++：
attach 扇出、`DetachShard` 驱逐集合、`GlobalFanout` 目标枚举（archive/branch/drop/
reopen）、archive crond `tick`、`PrewarmState::plan`。在 fkv_config 的 INI 面保持
"进程内回调，非配置文件项"，与 C++ 同。

## 严重度
P2

## 核验记录
- `grep -rn partition_filter fkv/crate --include='*.rs'`：仅 fkv_config 注释一处。
- 读 `fkv_runtime/src/lib.rs`：`shard()`（1279-1282）、`enumerate_targets`（1202-1214）、
  `ArchiveCrond::tick` attached 过滤（312-318）、`PrewarmState::plan`（470-505）。
- 读 C++ `eloq_store.cpp` 上述行号与 `shard.cpp:773-852`（EvictDsShard）、
  `prewarm_task.cpp:505-520`、`eloq_store.h:995-1040` 注释。
- 对照 `eloq_store.cpp:1020` 默认路由与 `fkv_types::shard_index`：取模语义一致。
