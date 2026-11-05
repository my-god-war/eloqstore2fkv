# eng-61 shard 线程初始化失败静默吞掉；无 Stop→Start 重启路径；Drop 不等待清理完成

## 位置
- Rust: `crate/engine/fkv_shard/src/lib.rs:105-112`（线程内 `let Ok(runtime) = Runtime::new() else { error!(...); return; }` —— `Shard::start` 已返回 `Ok`，失败后所有请求只得 `NotRunning`，无错误上报）；`crate/engine/fkv_runtime/src/lib.rs:727-751`（`shutdown(mut self)` 消耗 self；`Drop for Runtime` 只停 crond 不 join shard 线程；无重启接口）；`fkv_shard/src/lib.rs:269-276`（`shutdown` 同样消耗 self）
- C++: `src/eloq_store.cpp:857-868`（`StartInstance` 中 shard `Init()` 失败立即 `fail_start(err)` 返回具体错误）；`:2581-2623,569-659`（`Stop()` CAS 状态机 → CleanupRuntime 串行：NotifyStoreStopping → stop prewarm/archive → shard Stop（join）→ 关 root_fds → 停 standby/cloud → 重置；状态回 `Stopped` 后可再次 `Start`，`:661-684`）；`:556-567`（析构自动 `Stop()`）

## 现状
- compio runtime 创建失败延迟到线程内部，调用方拿到假成功；
- 进程内 shard 挂死后无法就地恢复（C++ 的 Stop/Start 循环是 standby/cloud 故障切换的基础路径）；
- 忘调 `shutdown()` 直接 drop 时无法观察清理完成时机。

## 差异与风险
长生命周期进程的可用性与 C++ 不对齐；启动失败排障困难。

## 对齐建议
1. `Shard::start` 创建线程后握手确认 runtime 初始化成功再返回（channel 回传 init 结果）；
2. 提供 shutdown 后重建 shard 的 restart 能力，或文档化「一次性」为有意偏离；
3. `Drop` 至少 drain-join 或显式提示未 shutdown。

## 严重度
P2（启动静默失败为高优先子项；重启语义属 API 面缺口，与 issue 12 相邻不重叠）

## 核验记录（2026-11-04）
Rust 侧线程内 `let Ok(...) else return`、消耗 self 的 shutdown 实读确认；C++ StartInstance fail 路径实读 `eloq_store.cpp:857-868` 引用，Stop/Start 循环引用自子代理摘录。

## 审查结论（2026-11-04）

### 核验（对照当前源码）

| 论断 | 代码证据（当前） | 判定 |
| --- | --- | --- |
| 线程内 `let Ok(runtime) = Runtime::new() else { error!; return; }`，`Shard::start` 仍返回 `Ok` | 改前 `crate/engine/fkv_shard/src/lib.rs:105-112` | 属实 |
| `shutdown(mut self)` 消耗 self，无重启接口 | `fkv_shard/src/lib.rs:284-290`、`fkv_runtime/src/lib.rs:729-744` | 属实 |
| `Drop for Runtime` 只停 crond，不 join shard 线程 | `fkv_runtime/src/lib.rs:747-753` | 属实 |
| C++ `StartInstance` Init 失败即 `fail_start(err)`；Stop/Start 可循环 | `cpp/` 只有 `build`/`shim`，无 include/src，仓内不可核验 | 采信语义依据 |

### 处置

- **子项 1（启动静默失败）采纳**：`Shard::start` 增加启动握手，线程内
  Runtime + store 就绪后才回 `Ok`，失败（含线程 panic 早退）返回 `Err(IoFail)`，
  `Runtime::start` 随之失败。见 DONE。
- **子项 3（Drop 不等待清理）部分采纳**：新增 `Drop for Shard`，未 `shutdown`
  即 Drop 时显式告警。不做 Drop 内 join：`tx` 非 `Option`，Drop 里必须先断开队列
  才能 join，否则 join 永远等不到退出；把 `tx` 改成 `Option` 会给 `sender()` 引入
  `expect`，且 Drop 可能落在 async poll 栈上，join 会阻塞在途长请求
  （auto_reopen 退避最长 100s）。队列发送端断开后线程本就会退出、资源会释放，
  缺的只是「完成时机」，用告警暴露即可。
- **子项 2（Stop→Start 重启）拒绝**：Rust 侧 `shutdown` 消耗 `self` 是类型级保证
  （shutdown 后无法再发请求），重启等价于重新 `Runtime::start`。C++ 复用实例是靠
  内部状态机 + 线程池，Rust 侧无对等结构。当前无调用方需要就地重启；standby /
  cloud 故障切换在 `fkv_cloud` 层重建 Runtime 即可。若真要支持，应新增 API 而不是
  把 `shutdown` 改成 `&mut self`（会放宽 19 处调用的生命周期约束）。

## DONE（2026-11-04）

### 实现要点

1. 启动握手：`sync_channel::<Result<()>>(1)`，shard 线程在 `Store`/事件循环就绪后
   回传 `Ok`；`Runtime::new()` 失败回传 `Err(KvError::IoFail)` 并带 `error=` 日志。
   `Shard::start` 阻塞等待握手结果，失败时 `join` 线程句柄后返回错误 —— 不再有假成功。
2. 线程 panic / 提前退出：`ready_rx.recv()` 出错按初始化失败处理，同样回收句柄。
3. `Drop for Shard`：未 `shutdown` 就 Drop → `warn!` 提示（`shard dropped without
   shutdown shard_id=`），不 join（理由见审查结论）。

### 改动文件

- `crate/engine/fkv_shard/src/lib.rs`

### 验证

- `cargo nextest run -p fkv_shard -p fkv_runtime`：23 → 20 passed，1 skipped，0 failed
- `cargo clippy -p fkv_shard -p fkv_runtime --no-deps -- -D warnings`：0 警告

（首次运行时 `fkv_runtime::ttl_cleanup` 因并发的 eng-64 TTL 下推改动处于半成品状态
而失败，30s 后重跑全绿，与本改动无关。）

## 复审（2026-11-05 r6）

**通过（子项 1、3；子项 2 维持拒绝）。** 核实：`fkv_shard/src/lib.rs:464` `sync_channel::<Result<()>>(1)` 启动握手、:477/:499 失败回传 `Err`（Runtime 失败 / init 失败）、:564 就绪回传 `Ok`；:855 Drop 未 shutdown 的 `warn!("shard dropped without shutdown ...")`。子项 2（Stop→Start 重启）拒绝理由（shutdown 消耗 self 为类型级保证）在 r6 仍成立。eng-32b 读 spawn 改造未破坏握手路径（ready_tx 逻辑保留）。
