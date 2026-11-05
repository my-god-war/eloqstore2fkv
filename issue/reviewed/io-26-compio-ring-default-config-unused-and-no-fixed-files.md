# io-26 compio ring 全用默认参数（capacity 1024、无 single_issuer/defer_taskrun），且未启用 fixed file，与 C++ ring 配置差距明显

## 位置
- Rust: `crate/engine/fkv_shard/src/lib.rs:106`（`Runtime::new()` 默认 `ProactorBuilder`，无 capacity / 无 task-run 调优）、`crate/io/fkv_io_compio/src/lib.rs:45`（`FD_CACHE_LIMIT: usize = 256` 硬编码，不接 `KvOption`/`ShardConfig`）、`:605-654`（读/写句柄普通 fd，未用 fixed file）
- C++: `eloqstore/src/async_io_manager.cpp:219-223`（`io_uring_queue_init_params(io_queue_size, ...)` + `IORING_SETUP_SINGLE_ISSUER | IORING_SETUP_DEFER_TASKRUN | IORING_SETUP_TASKRUN_FLAG`）、`:231-251`（`io_uring_register_files_sparse(fd_limit_)`，`fd_limit` 由 `AsyncIoManager::Instance(store, fd_limit)` 传入）、`:744-745 / 843-846 / 987-990 / 2229-2246`（`openat2_direct` + 每 SQE `IOSQE_FIXED_FILE`）、`:1967-2032`（`Submit` 每 shard 循环一次 `io_uring_enter` 聚合提交）
- compio: `compio-driver-0.12.5/src/lib.rs:583-601`（`ProactorBuilder::default`：`capacity: 1024`、`single_issuer/coop_taskrun/taskrun_flag/defer_taskrun` 全 false）、`:707-752`（对应 setter 均存在）、`compio-runtime-0.12.6/src/lib.rs:365-376, 647-664`（`Runtime::register_files / unregister_files` 已暴露；compio 自身 op 不使用 fixed file，需业务侧对 raw fd 提交时自行置 `IOSQE_FIXED_FILE`）

## 现状
每个 shard 的 compio Runtime 用默认参数创建：SQ 深度 1024、未开 SINGLE_ISSUER / DEFER_TASKRUN / TASKRUN_FLAG。FD 缓存上限硬编码 256，不可按部署调；所有读写 SQE 走普通 fd。C++ 的 ring 配置（可配 SQ 深度、deferred task-run 降低 task_work 开销、fd 槽位上限接配置）在 Rust 侧全部缺失。

## C++ 实现
- SQ 深度来自 `KvOption::io_queue_size`；`DEFER_TASKRUN+TASKRUN_FLAG` 把 task_work 延迟到显式 `io_uring_enter(GETEVENTS)` 时集中执行，配合单 issuer 假设减少原子操作（`async_io_manager.h:1052-1065` 还处理了 defer_taskrun 的唤醒竞态）；
- 文件通过 `register_files_sparse(fd_limit)` 注册，`IOSQE_FIXED_FILE` 提交，省掉每次 IO 的 fd 表引用计数与 fget/fput；`fd_limit` 超限走 `EvictFD()` LRU 驱逐（`2671-2688`）并映射 `EMFILE → OpenFileLimit`。

## 差异与风险
- **SQ 深度不可配**：高并发点读/批量写超过 1024 in-flight 时 compio 会在 `push_raw` 里循环 drain + 多付一次唤醒（`compio-driver .../iour/mod.rs:352-380`），尾延迟抖于 C++（`io_queue_size` 在 C++ 默认更大且可调）。
- **无 DEFER_TASKRUN**：C++ 明确靠它压 task_work 开销（thread-per-core 场景收益最大的参数之一），Rust 未启用即放弃这部分 p99 收益。
- **fixed file 未用**：每页读多付 fget/fput + fd 表缓存行竞争；compio 的 `register_files` 只能注册整表，且需自定义 op 置 `IOSQE_FIXED_FILE`——当前完全未使用，fd_limit 语义也未暴露（硬编码 256 与 C++ 每 shard `fd_limit` 配置不对齐，cloud 大表可能提前触发淘汰抖动）。
- compio `Driver::poll` 的 `submit_auto` 在空队列时可能睡到下一个事件（`:185-224`），与 C++ 的 `kForceSubmitEveryNoOps` 强制 enter 策略不同；compio 用 notifier PollAdd 机制自身保证唤醒，这条仅提示不要误以为需要移植 C++ hack。

## 对齐建议
- `fkv_shard` 改用 `Runtime::builder().proactor_builder(ProactorBuilder::new().capacity(config.io_queue_size).single_issuer(true).taskrun_flag(true).defer_taskrun(true))`，参数接 `KvOption`（对齐 C++ `io_queue_size`）。
- `FD_CACHE_LIMIT` 从 `ShardConfig`/`KvOption` 传入；`CompioStore::new` 接收 `fd_limit`。
- 评估 fixed file：compio 暴露 `register_files`，但 compio 自身 `File` op 不带 `IOSQE_FIXED_FILE`，需仿照 `fixed.rs` 用自定义 OpCode 包一层 `Read/Write + IOSQE_FIXED_FILE`，配合 FD 缓存的注册槽位管理（对齐 C++ `AllocRegisterIndex/FreeRegisterIndex`）后再启用；若收益不确定，至少把 fd_limit 语义对齐并补淘汰路径的 `OpenFileLimit` 分类（io-22）。

## 性能优化建议
DEFER_TASKRUN + SINGLE_ISSUER 是 thread-per-core 下的已验证收益点（C++ 实测依赖）；容量按 `max_inflight_write + max_read_pages_batch * scan 并发` 量级估算，避免默认 1024 成为隐式瓶颈。

## 严重度
P2（性能 / 配置缺失）

## 审查结论（2026-11-04）

**部分采纳（配置透传优先，fixed file 缓做）**。

- 成立：`Runtime::new()` 默认 1024 容量，未开 SINGLE_ISSUER / DEFER_TASKRUN；
  `FD_CACHE_LIMIT=256` 硬编码不接配置。
- 先做：`fkv_shard` 用 `ProactorBuilder` 接 `io_queue_size`；FD 上限接
  `KvOption`（需先在 `fkv_config` 增加字段并透传）。
- fixed file 收益不确定且需自定义 OpCode + `IOSQE_FIXED_FILE`，暂缓；
  与 io-25 的 ring 生命周期绑定一起评估。
- 阻塞：`fkv_shard` 正由 eng-56 修改；`fkv_io_compio` 正由 io-06 修改。

## 进展（2026-11-04，部分完成）

- `FdCache` 增加 `limit` 字段，默认 `FD_CACHE_LIMIT=256`；
  `CompioStore::with_fd_cache_limit(limit)` 可在构造时覆盖，`0` 表示不限制。
- 新增 `hardening::fd_cache_limit_is_configurable`：limit=2 时写 5 个
  file id，缓存条数保持 ≤ 2。
- 验证：`cargo nextest run -p fkv_io_compio` 28 passed。

未做：`fkv_shard` 的 `ProactorBuilder`（capacity/single_issuer/defer_taskrun）
与 `KvOption::fd_limit`/`io_queue_size` 透传；fixed file 需自定义 OpCode。

## 审查结论（2026-11-04，二次审查）

**部分采纳：ring 配置与 fd_limit 透传落地；fixed file 明确跳过并记录原因。**

核验结果：

- 成立：`ShardConfig` 无 `io_queue_size`（原 `:106` 行号已漂移），shard 线程
  `Runtime::new()` 用默认 1024，未开 `SINGLE_ISSUER / DEFER_TASKRUN /
  TASKRUN_FLAG`。`fkv_config::KvOption::io_queue_size`（默认 4096）与
  `DEFAULT_IO_QUEUE_SIZE` 均无消费方。
- 已成立但此前未修：`FdCache::insert` 的 `while len > limit` 在 `limit == 0`
  时把缓存全部淘汰，与 `with_fd_cache_limit(0)` 文档「0 表示不限制」相反。
  本轮一并修复（`fkv_runtime` 在 `fd_limit=0` 时正是传 0）。
- fixed file：compio 0.19 只暴露 `Runtime::register_files / unregister_files`
  （整表注册），`compio_fs::File` 的 `Read/Write` op 不置 `IOSQE_FIXED_FILE`。
  `fixed.rs` 现有自定义 op 是 `READ_FIXED/WRITE_FIXED`（注册缓冲），不能直接
  复用。启用 fixed file 需新写 `Fd(N)` + `IOSQE_FIXED_FILE` 的 op，并让
  `FdCache` 的 LRU 淘汰与在飞 op 的槽位生命周期严格同步（句柄 clone 在飞时
  不能 unregister）。收益不确定、竞态面大，**跳过**，另立 issue，不在本轮扩范围。

对齐口径：

- `build_runtime` 首次按 `SINGLE_ISSUER + DEFER_TASKRUN + TASKRUN_FLAG` 建
  ring；失败（内核 < 6.1 / 非 io_uring）回落为仅设置 capacity 的 ring 并 warn。
  C++ `BootstrapRing` 失败即返回 `IoFail`，Rust 选择可启动优先，差异已记录。
- `io_queue_size` 从 `KvOption` 经 `fkv_runtime` 透传到 `ShardConfig`，单值
  驱动 `ProactorBuilder::capacity`，不新增平行配置常量。

## todo（2026-11-04）

1. `fkv_config`：新增 `DEFAULT_IO_QUEUE_SIZE = 4096` 并替换 `Default` 字面量；
   `validate` 拒绝 `io_queue_size == 0`。
   验收：`cargo check -p fkv_config`。
2. `fkv_shard`：`ShardConfig.io_queue_size: u32`；`build_runtime(io_queue_size)`
   用 `ProactorBuilder` 设置 capacity / single_issuer / defer_taskrun /
   taskrun_flag，失败回落 capacity-only。
   验收：`Shard::start` 各集成测试通过。
3. `fkv_runtime`：从 `KvOption` 取 `io_queue_size`（无 options 用默认值）写入
   `ShardConfig`。
   验收：`cargo nextest run -p fkv_runtime`。
4. `fkv_io_compio`：`FdCache` 的 `limit == 0` 语义改为不淘汰，补测试
   `hardening::fd_cache_limit_zero_is_unlimited`。
   验收：limit=0 写 5 个 file id 后 `fd_cache_len() == 5`。

## DONE（2026-11-04）

实现：

- `crate/foundation/fkv_config/src/lib.rs`：`DEFAULT_IO_QUEUE_SIZE = 4096`，
  `KvOption::default` 引用该常量；`validate` 拒绝 `io_queue_size == 0`。
- `crate/engine/fkv_shard/src/lib.rs`：`ShardConfig.io_queue_size: u32`；
  新增 `build_runtime(io_queue_size)`，`ProactorBuilder::capacity` +
  `single_issuer(true)` + `defer_taskrun(true)` + `taskrun_flag(true)`；
  构建失败回落 capacity-only 并 warn（内核 < 6.1 / 非 io_uring）。
- `crate/engine/fkv_runtime/src/lib.rs`：`io_queue_size` 从 `KvOption` 透传，
  无 options 时用 `DEFAULT_IO_QUEUE_SIZE`。
- `crate/io/fkv_io_compio/src/lib.rs`：`FdCache` 在 `limit == 0` 时不再淘汰，
  与 `with_fd_cache_limit` 文档一致。
- `crate/io/fkv_io_compio/tests/hardening.rs`：新增
  `fd_cache_limit_zero_is_unlimited`。
- `crate/engine/fkv_shard/tests/*.rs`：4 处 `ShardConfig` 字面量补
  `io_queue_size: 4096`。

验证：

- `cargo check -p fkv_io_compio -p fkv_shard -p fkv_runtime`：通过。
- `cargo nextest run -p fkv_io_compio -p fkv_shard -p fkv_runtime --no-fail-fast`：
  76 passed，1 skipped。
- `cargo clippy -p fkv_io_compio -p fkv_shard --no-deps --all-features --all-targets -- -D warnings`：零警告。
- `cargo clippy -p fkv_runtime -p fkv_config --no-deps --all-features --all-targets -- -D warnings`：零警告。

遗留：

- fixed file 未启用：compio 0.19 的 fs op 不带 `IOSQE_FIXED_FILE`，需自定义
  OpCode 并与 `FdCache` LRU 淘汰 / 在飞句柄生命周期绑定；另立 issue。
- Linux 真内核下 `SINGLE_ISSUER + DEFER_TASKRUN` 的 p99 收益未在本机（macOS）
  验证，需 Linux 压测确认；老内核走 capacity-only 回落路径。

## 复审（2026-11-05 r6）

通过。

- `fkv_config/src/lib.rs:47 DEFAULT_IO_QUEUE_SIZE=4096`，`:314 validate` 拒绝
  `io_queue_size == 0`；`fkv_shard/src/lib.rs:236-246 build_runtime` 设
  capacity/single_issuer/defer_taskrun/taskrun_flag，失败回落 capacity-only
  （`:258-261`），`Shard::start` 消费（`:468`）；`fkv_runtime/src/lib.rs:1096-1097`
  从 KvOptions 透传、无 options 用默认。
- `FdCache` `limit == 0` → `usize::MAX` 不淘汰（`fkv_io_compio/src/lib.rs:908-911`）；
  `tests/hardening.rs:243 fd_cache_limit_zero_is_unlimited` 断言 5 句柄全保留
  （`:254`）、`:226 fd_cache_limit_is_configurable` 断言 ≤2（`:238`）。
- `FD_CACHE_LIMIT` 已由 256 演进为 10_000（`:54-56`，eng-79b fd_limit 接线），
  对齐 C++ 默认，属后续正式修正。
- fixed file 明确跳过（compio fs op 无 IOSQE_FIXED_FILE，需自定义 OpCode + 与
  FdCache 淘汰生命周期绑定），视为已决议，未立项无异议。
