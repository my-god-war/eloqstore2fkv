# perf-83 parking_lot 出现在 fkv_object 云层，偏离「仅生命周期/Attach」锁策略

## 位置
- Rust: `crate/io/fkv_object/src/lib.rs:8`（`use parking_lot::Mutex`）、`lib.rs:132-135`（`Arc<Mutex<ObjectMap>>` 等三个 mutex）、`lib.rs:144-182`（`map.lock()` 每次操作加锁）
- todo.md: `todo.md:267`（「锁 parking_lot 仅 Store 生命周期、Attach/Detach；shard 热路径无锁」）、`todo.md:315`（shard 内 `Rc`/`Cell`/`RefCell`，不实现 Send/Sync）

## 现状
- `fkv_object` 的 `ObjectStore` 实现用 `Arc<Mutex<ObjectMap>>`、`Arc<Mutex<u64>>` 等三个 `parking_lot::Mutex` 保护对象元数据缓存（`lib.rs:132-135`）。
- 每次 `get`/`put`/`len` 都 `self.map.lock()`（`lib.rs:144-182`）。
- 该 crate 是 `fkv_object_s3`（S3/RustFS）的上层，云模式整文件上传/下载路径。

## C++ 实现 / 目标口径
- C++ `AsyncIoManager`/云层对象缓存用 shard 内单线程状态或 lock-free 结构，不在云对象元数据上用全局 mutex。
- todo.md 锁策略：`parking_lot` 仅 Store 生命周期、Attach/Detach；shard 热路径无锁。

## 差异与风险
- **锁策略偏离**：`fkv_object` 在云对象元数据缓存上用 `parking_lot::Mutex`，不在 todo.md 允许的「生命周期/Attach」范围内。
- 云模式整文件上传/下载时，每次对象元数据访问加锁，并发上传多个对象时锁竞争。
- 三个独立 mutex（map/version/range_put_bytes）增加锁开销与潜在死锁面。
- 云模式性能（上传字节数、吞吐）验收无明确门槛，但锁竞争会拖慢并发上传。

## 对齐建议
- 云对象元数据改用 shard 内单线程状态（`Rc`/`RefCell`）或 `papaya`（todo.md 选型：papaya 用于全局路由/跨 shard 只读表），避免 mutex。
- 或明确把 `fkv_object` 锁纳入「生命周期」例外，更新 todo.md 锁策略文档。
- 合并三个 mutex 为一个 `ObjectState`，减少锁次数。

## 性能优化建议
- 元数据读多写少，用 `RwLock` 或 `papaya` 读优化结构。
- 上传路径批量取元数据，减少 `lock()` 次数。

## 严重度
P2（性能优化，云模式并发上传风险）

## 审查结论（2026-11-04）

**不成立，关闭（锁不在生产热路径）。**

### 逐条核验

| 原陈述 | 现状 | 结论 |
|---|---|---|
| `fkv_object/src/lib.rs:8,132-135` 三个 `parking_lot::Mutex` | 属实：`map` / `version` / `range_put_bytes` | 存在，但不是热路径 |
| `lib.rs:144-182` 每次 get/put/len 加锁 | 属实：`len`/`is_empty`/`get`/`get_versioned`/`put`/`put_range`/`head`/`delete`/`list` 均短临界区内 `lock()` | 锁存在，但对象是测试替身 |
| 「该 crate 是 `fkv_object_s3` 的上层，云模式整文件上传/下载路径」 | **不成立**。`fkv_object_s3` 直接 `impl ObjectStore for S3Store`，零 `Mutex`；`fkv_object` 只有 trait 与 `MemObjectStore` | 无生产云路径使用这些锁 |
| `MemObjectStore` 处于云模式生产路径 | **不成立**。全仓 `MemObjectStore` 的非测试消费者为 0：`rg -g '!tests/'` 仅命中自身定义；`fkv_cloud` / `fkv_task_prewarm` / `fkv_task_reopen` 只在 `tests/` 里构造 | 纯测试替身 |
| `fkv_cloud` 云缓存锁竞争 | **不成立**。`crate/engine/fkv_cloud/src/lib.rs` 全文无 `Mutex`/`RwLock`；cld-51 / cld-51a 已改为侵入式 LRU（`Vec<LruNode>` + `head/tail` 索引，O(1)），单线程 `&mut` 访问 | 无锁可退 |
| 三个 mutex 存在死锁面 | 不存在：`put`/`put_range` 的获取序恒为 `map` → `version`，`range_put_bytes` 只在持 `map` 时获取，无任何反向获取路径 | 无死锁 |
| 违反 `todo.md:267`「锁仅 Store 生命周期、Attach/Detach」 | **口径需修正而非改代码**：该条约束的是 shard 热路径；`MemObjectStore` 是可克隆、跨测试任务共享同一状态的内存实现；改 `Rc`/`RefCell` 会丢掉 `Send + Sync`，破坏 `ObjectStore` 后端可替换契约，不能改 | 不属于违规 |

### 判定依据

1. 锁在测试替身内，不在任何生产上传/下载路径；并发上传的真实后端是 `fkv_object_s3`（无锁，`object_store` 内部 tokio 线程）。
2. `parking_lot` 用法本身正确：无 `await` 跨临界区、临界区短、无嵌套反向获取。
3. 建议项「改 `Rc`/`RefCell`」与 `ObjectStore` 的共享契约冲突；「改 `papaya`」按 `todo.md` 仅用于全局路由/跨 shard 只读表，套到单机测试替身上属于过度设计。
4. 「合并三个 mutex 为一个 `ObjectState`」在测试替身上只减一次原子操作，收益低于改动风险，且会扩大锁持有范围（把 bytes 计数也纳入），不立项。

### 若后续形态变化

若 `MemObjectStore` 将来被提升为生产内存后端（当前无调用方），再按单 `Mutex<ObjectState>` 合并并单独立项；本 issue 不预留 TODO。
