# 32 P1：任务抽象层缺失，fkv_task / compaction / archive 等是空壳

## 现象

以下 crate 的 `src/lib.rs` 只有一行文档注释，无任何代码：

| crate | 设计职责 | 当前 |
|-------|----------|------|
| `fkv_task` | `Task` trait / `TaskCtx` / 等待座位 / 协程锁 / yield / 重试 | 空壳 |
| `fkv_compaction` | 空间放大分析、页 / segment 重写计划 | 空壳 |
| `fkv_task_archive` | Archive / CreateBranch / DeleteBranch 任务 | 空壳（逻辑内联在 `fkv_store`） |
| `fkv_failpoint` | 命名 fail / kill point 注册表 | 空壳（另见 `17-crash-fuzz-failpoints.md`） |
| `fkv_io_sim` | 确定性模拟后端（延迟 / 故障 / crash / 重排） | 空壳 |
| `fkv_testkit` | 临时目录 / 夹具 / 确定性时钟 / crash harness | 空壳 |

其中 `fkv_task` 最关键：todo.md 把「任务模型」列为核心机制（future 任务、任务注册表、每分区写队列、yield / 优先级门），但**没有 `Task` trait**。

## C++ 对照

C++ 有 `KvTask` 基类 + `TaskManager` + `WaitingZone` / `WaitingSeat` + 优先级队列（`include/tasks/task.h:206-282`、`src/tasks/task_manager.cpp:121-153`）。这是 shard 内多任务并发调度的基础。

## 影响

- `fkv_shard` 没有任务抽象，只能串行 `handle().await`（见 `03-shard-request-concurrency.md`）。
- 没有等待座位 / 协程锁，「每分区写队列」「优先级门」无处落地。
- `17`（crash / fuzz / failpoint）依赖 `fkv_failpoint` / `fkv_io_sim` / `fkv_testkit`。

## 建议

1. 先落 `fkv_task`：`Task` trait + `TaskCtx` + 等待座位 + yield + 优先级门。
2. `fkv_failpoint` / `fkv_io_sim` / `fkv_testkit` 排 M7 前，是 V5 崩溃恢复与确定性测试的基础。
3. `fkv_compaction` 抽出 `fkv_store` 里的 compaction 编排。
4. `fkv_task_archive` 抽出内联的 branch / archive 逻辑。

## 验收

- `fkv_task` 有 trait 与单测；`fkv_shard` 通过它调度多任务。
- 空壳 crate 数归零，或明确标注 deferred。

## 审查结论（2026-11-04）

### 总结论

**部分采纳**。6 个 crate 中 1 个已实现（`fkv_failpoint`，误报）、2 个有独立价值（`fkv_io_sim`、`fkv_testkit`）、3 个职责已被别处覆盖（`fkv_task`、`fkv_compaction`、`fkv_task_archive`）。

关键判断：

- 「空壳」本身属实（5/6 只有 1 行 doc comment），但「缺任务抽象层」的诊断不对。

  Rust 侧已经有 7 个具体任务 crate（`fkv_task_read`/`scan`/`write`/`compact`/`gc`/`reopen`/`prewarm`，共约 2200 行），
  它们是纯 async 函数，不是 C++ `KvTask` 的对象模型。

  C++ `KvTask` 的 `Yield`/`WaitIo`/`WaitingZone`/`Mutex` 在 async/await 下大部分由 `Future` + `await` 天然承担。
  真正缺的部分（高/低优先 ready 队列、yield 预算、每分区写队列）**已分别由 `eng-32`、`eng-34`、`eng-33` 立案**。
  所以新造 `fkv_task` 只会引入一层与 `compio` 并行的死抽象。

- 因此本 issue 的正确收敛是**删壳**，不是**填壳**。

### 逐条核验表

| crate | 实测 | 原判断 | 结论 | 证据 |
|---|---|---|---|---|
| `fkv_task` | 1 行（只有 doc） | 空壳 | 空壳属实，但**职责重复**。`Task` trait/`TaskCtx`/等待座位在 Rust 无对应物；yield 预算见 `eng-34`，ready 队列见 `eng-32`，协程锁≈写门见 `eng-33` | `crate/engine/fkv_task/src/lib.rs:1`；`fkv_task_*` 7 个具体 crate 共 ~2200 行纯 async 函数；C++ `include/tasks/task.h:206-282`，`src/tasks/task_manager.cpp:121-153` |
| `fkv_compaction` | 1 行 | 空壳 | 空壳属实，但**已实现、无剩余职责**。分析在 `fkv_gc`，执行在 `fkv_task_compact`；`fkv_ledger` 无对应 C++ 映射 | `fkv_gc/src/lib.rs:27,35,52`（`file_amplification`/`should_compact`/`pick_compact_file`，77 行）；`fkv_task_compact/src/lib.rs:13`（`compact_file`，30 行）；`tool/fkv_ledger/src/map.rs` 无 compaction 条目 |
| `fkv_task_archive` | 1 行 | 空壳（逻辑内联 `fkv_store`） | 空壳属实，但**周期调度已在 `fkv_runtime`、请求级逻辑已在 `fkv_store`**。C++ 对照只是 `ArchiveCrond`，无独立任务类 | `fkv_runtime/src/lib.rs:80-160`（`ArchiveCrond`）；`fkv_store/src/lib.rs:1581,1606,1704,1723`；C++ `include/tasks/archive_crond.h:31`、`src/tasks/archive_crond.cpp`（403 行） |
| `fkv_failpoint` | 203 行（175 + 28 测试） | 空壳 | **误报，已实现并接线**。命名点注册表 + `@count`/`!kill`/`!err=` 语法 + 单测；`fkv_io_compio`、`fkv_store`、shard OOM 重试测试都在用 | `fkv_failpoint/src/lib.rs` 全文、`tests/failpoint.rs`；`rg fkv_failpoint` 命中 `crate/io/fkv_io_compio/src/lib.rs`、`crate/engine/fkv_store/tests/compio_manifest.rs`、`crate/engine/fkv_shard/tests/oom_retry.rs` |
| `fkv_io_sim` | 1 行 | 空壳 | **空壳属实，无替代，建议实现**。`fkv_io_mem` 只存内存，不做延迟/故障/重排/掉电；`fkv_failpoint` 是进程序内错误注入，无法模拟「已写未 fsync 的页在 crash 后丢失」 | `fkv_io_mem/src/lib.rs`（410 行，纯 HashMap）；`fkv_io/src/lib.rs` trait 面已足够承载 Sim 后端 |
| `fkv_testkit` | 1 行 | 空壳 | **空壳属实，无替代，建议实现**。仓库无 `tempfile`；39 个测试文件各自手抄一份 `temp_root`（`temp_dir` + pid + nanos + 计数器 + 手动 `remove_dir_all`） | `rg -c 'fn temp_root' crate` = 39 个文件；`crate/engine/fkv_store/tests/compio_store.rs:15-27`、`crate/io/fkv_io_compio/tests/hardening.rs:15-31` |

补充：`_cpp` 系列空壳（含 `fkv_task_cpp`/`fkv_compaction_cpp`/`fkv_io_sim_cpp`/`fkv_testkit_cpp`）是另一个问题，归 `fmt-16`，不在本 issue 范围。
`issue/new/00-index.md` 把本条列为 P2，与标题的 P1 不一致，以 P2 为准（删除动作除外，见下）。

### 采纳方案与开发计划

#### P1（先做，收敛，不新增抽象）

删除 3 个无剩余职责的空壳 crate，直接达成「空壳归零」：

1. 删 `crate/engine/fkv_task/`、`crate/engine/fkv_compaction/`、`crate/engine/fkv_task_archive/`。
2. 在 `doc/migration.md` / ledger 注明能力去向：
   - 任务调度语义 → `eng-32`（ready 队列）/ `eng-34`（yield 预算）/ `eng-33`（写门）
   - 空间放大分析 → `fkv_gc`；重写执行 → `fkv_task_compact`
   - 周期归档 → `fkv_runtime::ArchiveCrond`；请求级 archive/branch → `fkv_store`
3. `cargo check --workspace` 验证无引用（当前三者在 `Cargo.toml` 里均无依赖者，删除无涟漪）。

不做的事：不在 `fkv_shard` 外新建 `Task` trait。若 `eng-32` 落地后确需任务对象，就把 ready 队列/优先级门直接放在 `fkv_shard` 内部，与 work loop 同生共死。

#### P2（按价值排序）

1. `fkv_testkit` 最小实现：`TempRoot`（`Drop` 自动清理）、唯一 tag、确定性时钟、crash harness 骨架。

   迁移 39 处 `fn temp_root`，先迁 `fkv_store`/`fkv_runtime`/`fkv_io_compio` 三组。
   依赖用 `cargo add --path` 加到各测试 crate 的 `[dev-dependencies]`。

2. `fkv_io_sim`：基于 `PageIo`/`ManifestIo`/`SegmentIo` 的确定性后端。

   能力：可配置延迟、按概率/次数故障、写重排、未 fsync 数据在 `crash()` 后丢弃、固定种子 RNG（`fastrand::Rng::with_seed`）。
   验收：同一 seed 两次跑出逐字节一致的调用序列与最终盘面。

#### P3（defer）

`fkv_task_archive` 若后续 `fkv_store` 继续膨胀（现 2249 行），可再抽成自由函数模块（参照 `fkv_task_compact` 的 `(&I, &TableIdent, ...)` 形态），但**不复活空 crate**。

### 验收（修订）

- `fkv_task` / `fkv_compaction` / `fkv_task_archive` 三个空目录消失，ledger 与文档记录去向。
- `fkv_failpoint` 条目关闭（已实现）。
- `fkv_testkit` 提供 `TempRoot` 且 `fkv_store` 等测试改用之，手抄 `temp_root` 数量降到 0（或仅剩 testkit 内一份）。
- `fkv_io_sim` 有 seed 决定性的失败/崩溃注入测试通过。
- `fkv_shard` 的并发调度验收仍归 `eng-32`/`eng-33`/`eng-34`，本 issue 不重复认领。

## 进展（2026-11-04，收敛完成）

- 删除 6 个空壳 crate：
  `fkv_task`、`fkv_task_cpp`、`fkv_compaction`、`fkv_compaction_cpp`、
  `fkv_task_archive`、`fkv_task_archive_cpp`。
- 职责去向：
  - `task.h` → `fkv_shard` 调度循环 + 各 `fkv_task_*` 具体任务；
  - `archive_crond.*` → `fkv_runtime::ArchiveCrond`；
  - compaction 分析在 `fkv_gc`，执行在 `fkv_task_compact`。
- `tool/fkv_ledger/src/map.rs` 对应映射已改指 `fkv_shard` / `fkv_runtime`。
- 验证：`cargo check --workspace --all-features`、`./test.sh`
  318 passed / 1 skipped。

仍待做：`fkv_testkit`（39 处手抄 `temp_root`）、`fkv_io_sim`（确定性故障注入）。

## 复核结论（2026-11-04，第二轮）

### 实测状态

| crate | 现状 | 结论 |
|---|---|---|
| `fkv_task` | 目录已不存在 | 已收敛，职责归 `fkv_shard` + `eng-32/33/34` |
| `fkv_compaction` | 目录已不存在 | 分析在 `fkv_gc`，执行在 `fkv_task_compact` |
| `fkv_task_archive` | 目录已不存在 | 周期归档在 `fkv_runtime::ArchiveCrond`，请求级在 `fkv_store` |
| `fkv_failpoint` | 203 行，已接线 | 原判断误报 |
| `fkv_testkit` | 本轮实现 `TempRoot`（89 行 + 3 个集成测试） | 空壳已消除，剩余「迁移调用点」拆分 |
| `fkv_io_sim` | 仍 1 行 doc | 真空壳，拆分待办 |
| `*_cpp` 系列 | 仍 1 行 | 归 `fmt-16`，不在本条范围 |

文档同步：`todo.md` 删除 `fkv_task` / `fkv_compaction` / `fkv_task_archive` 条目并改正任务 crate 依赖；
`doc/architecture/{01-overview,02-execution-model,06-gaps}.md` 去掉已删 crate 与「尚未引入任务抽象」的错误表述。

### 判定

**合理，但已大部分收敛**。原「缺任务抽象层」的诊断不成立（Rust 用 async 函数 + work loop，
不引入 C++ 的 `Task` 对象模型）；剩余真实工作只有两项，已拆为：

- `32a-io-sim-deterministic-backend.md`：实现 `fkv_io_sim`（确定性故障 / 掉电后端）。
- `32b-migrate-testkit-temp-root.md`：把 50 处手抄 `temp_root` 迁到 `fkv_testkit::TempRoot`。

本文件作为背景与决策记录保留在此。

## DONE（2026-11-04）

- P1 收敛完成：`fkv_task` / `fkv_task_archive` / `fkv_compaction`（含 `_cpp`）目录已删除，
  职责去向写入 `tool/fkv_ledger/src/map.rs`、`todo.md`、`doc/architecture/*`。
- `fkv_testkit` 空壳已消除：`TempRoot` + 3 个集成测试（`cargo nextest run -p fkv_testkit` 全绿）。
- `fkv_failpoint` 误报关闭。
- 剩余工作已拆出：`issue/todo/32a-io-sim-deterministic-backend.md`（`fkv_io_sim` 实现）、
  `issue/todo/32b-migrate-testkit-temp-root.md`（50 处 `temp_root` 迁移）。
- 验证：`cargo check --workspace --all-targets --all-features`、
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`、
  `cargo nextest run --workspace --all-features`（466 passed / 1 skipped）。

## 复审（2026-11-05 r6b）

通过。

- 6 个空壳 crate（`fkv_task`/`fkv_compaction`/`fkv_task_archive` 及 `_cpp`）已删：`crate/engine/` 目录列表无对应目录（仅剩 fkv_task_{read,scan,write,compact,gc,reopen,prewarm} 及 `_cpp`）。
- `fkv_testkit` 空壳已消除：`crate/facade/fkv_testkit/src/lib.rs:27`（`TempRoot` + `:66 Drop`）、`tests/temp_root.rs` 集成测试在。
- ledger 映射改指：`tool/fkv_ledger/src/map.rs:51`（`task.h → fkv_shard`）、`:63`（`write_task.h → fkv_task_write`）。
- 拆分件 `32a`（io_sim，1309 行实现）/`32b`（testkit 迁移）已由其他 lane 完成并入 `issue/reviewed/`。
