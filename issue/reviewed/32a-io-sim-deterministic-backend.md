# 32a 实现 `fkv_io_sim`：确定性模拟 / 故障 / 掉电后端

## 来源

`issue/done/32-stub-crates-missing-task-layer.md` 拆分项。原 issue 的 6 个空壳 crate 中
5 个已收敛（3 个删除、`fkv_failpoint` 早已实现、`fkv_testkit` 本轮落地 `TempRoot`），
只剩 `crate/io/fkv_io_sim/src/lib.rs` 是 1 行 doc 空壳。

## 现状

```
crate/io/fkv_io_sim/Cargo.toml   # [dependencies] 为空（注释占位）
crate/io/fkv_io_sim/src/lib.rs   # 仅 //! 一行
```

全仓无任何 crate 依赖它；`issue/todo/33-orphan-crates.md` 已把它标为 `test-only` 白名单，
`issue/todo/io-03-...md` 把它列为本 lane 允许 crate。属「将要被使用」，不删。

## 职责（缺口）

- `fkv_io_mem`：纯 HashMap，无延迟 / 故障 / 重排 / 掉电语义。
- `fkv_failpoint`：进程内命名点注入错误，无法表达「已写未 fsync 的页在 crash 后丢失」。

需要的是**确定性、可复现**的故障后端，供 V5 崩溃恢复与 `17-crash-fuzz` 使用。

## 方案

位置：`crate/io/fkv_io_sim/src/lib.rs`，集成测试 `crate/io/fkv_io_sim/tests/sim.rs`。

### 结构

```rust
pub struct SimStore { /* Mutex<SimState> + 配置 + 计数器 */ }

struct SimState {
  durable: Layer,   // 已 fsync
  volatile: Layer,  // 已写未同步
  op_log: Vec<Op>,
  clock: u64,       // 逻辑时钟，每 op +1（不读墙钟）
}
struct Layer { /* page / segment / manifest / manifest_file / term / file 六张 gxhash Map */ }
```

- 自持存储，不包装 `fkv_io_mem::MemStore`（其字段私有、无快照 API，避免跨 lane 改动）。
- 实现 `fkv_io` 的 `PageIo` / `ManifestIo` / `TermIo` / `SegmentIo` / `FileIo` / `FixedSegmentIo`
  六个 trait；`FixedSegmentIo` 用 `FixedBuffer::new` 的非注册形态（与 `MemStore` 一致）。
- 写一律落 `volatile`；`sync_data` 把 volatile 合并进 durable 并清空；
  `crash()` / `restart()` 丢弃 volatile，返回 durable 视图 → 模拟掉电丢未 fsync 数据。

### 可配置项（builder，默认全关，零开销路径不记录）

| 配置 | 语义 |
|---|---|
| `delay_li: Vec<(Op, u64)>` | 仅记录逻辑延迟到 `op_log`，不做墙钟 sleep（保持确定性、测试不慢） |
| `fault_li: Vec<Fault>` | `Fault { op, at: u32, err: KvError }`：第 `at` 次该 op 返回错误且不写状态 |
| `reorder_seed: Option<u64>` | 非空时 `sync_data` 按 `fastrand::Rng::with_seed` 打乱待刷写页顺序 |
| `op_log()` | 返回 `&[Op]`，供「同 seed 两次跑出同一序列」断言 |

`Op` 为 `#[derive(Debug, PartialEq, Eq)]` 的枚举（`ReadPage`/`WritePage`/`SyncData`/… 带参数）。

### 依赖

`cargo add --path` 加 `fkv_io` / `fkv_error` / `fkv_types` / `fkv_buf`；
`cargo add gxhash fastrand`（版本走 workspace 统一策略）。

## 验收

1. `SimStore::crash()`：未 `sync_data` 的页写丢失，返回旧值；`sync_data` 后 crash 保留新值。
2. 同一 `reorder_seed` 跑两次，`op_log()` 逐项相等且最终盘面逐字节相等。
3. `fault_li` 第 N 次命中返回指定 `KvError`，且该次写不改变状态。
4. `cargo nextest run -p fkv_io_sim`、`cargo clippy -p fkv_io_sim --all-targets -- -D warnings` 通过。
5. `crate/io/fkv_io_sim/src/lib.rs` 不再是 1 行空壳；`issue/todo/33` 的 `test-only` 白名单仍成立。

## 不做

- 不引入真实线程 / 真实延迟（会破坏确定性）。
- 不改 `fkv_io_mem` / `fkv_io`（并行 lane 可能占用）。

## DONE（2026-11-04）

位置：`crate/io/fkv_io_sim/src/lib.rs`（实现）、`crate/io/fkv_io_sim/tests/sim.rs`（集成测试）、
`crate/io/fkv_io_sim/Cargo.toml`（依赖）。

### 实现要点

- `SimStore` 自持两层：`durable`（已 fsync）+ `volatile`（未 fsync 覆盖层，`None` 为删除墓碑）。
  读走「volatile 覆盖 durable」；`sync_data` 把 volatile 合并进 durable 并清空；`crash()`/`restart()`
  丢弃 volatile，保留 durable。
- 六组 trait 全部实现：`PageIo` / `ManifestIo` / `TermIo` / `SegmentIo` / `FileIo` / `FixedSegmentIo`。
  `FileIo` 沿用 `MemStore` 语义从页表派生（`write_file` 拆页、`read_file` 重组），
  `FixedSegmentIo` 用 `FixedBuffer::new` 非注册形态。
- `Op`（带参数的枚举）+ `OpKind`（可按「第 N 次」匹配的类别）+ `OpRecord { op, clock, delay }`；
  逻辑时钟每 op +1，全程不读墙钟、不起线程、不 sleep。
- `delay_li`：只把逻辑延迟写进 `op_log`，不 sleep。`fault_li`：第 `at` 次该 `OpKind` 返回指定
  `KvError` 且不改状态（`at` 从 1 起算，0 永不命中）。`reorder_seed`：`sync_data` 前先对页键
  规范排序再 `fastrand::Rng::with_seed` 打乱，刷写顺序逐项记入 `op_log`（`FlushPage`/`FlushSegment`），
  使 reorder 可观察且可复现。
- term 立即落 durable（对齐 C++ `CURRENT_TERM` 写后 fsync），不参与 volatile 覆盖。
- 额外提供 `snapshot()`（排序去重的盘面视图，派生 `PartialEq`/`Eq`）供逐字节断言。
- `read_segment_fixed`/`write_segment_fixed` 先在独立作用域内记录 op 并释放锁，再 await 内层
  `read_segment`/`write_segment`，避免非重入 `Mutex` 自锁。

### 验收对照

1. `crash_drops_unsynced_and_keeps_synced`、`crash_drops_unsynced_manifest_and_delete_tombstone`：
   未 sync 的页 / manifest 写与删除墓碑在 crash 后丢失，sync 后保留。
2. `reorder_seed_is_deterministic`：同 seed 两次 `op_log` 逐项相等、`snapshot` 逐字节相等；
   不同 seed 只改刷写顺序不改最终盘面。
3. `fault_returns_error_and_keeps_state`：第 2 次 `WritePage` 返回 `IoFail` 且该写不落盘；
   `fault_at_zero_never_fires` 覆盖 `at == 0` 不命中。
4. `cargo nextest run -p fkv_io_sim` 13 passed；
   `cargo clippy -p fkv_io_sim --all-targets --no-deps -- -D warnings` 零警告；
   `cargo fmt -p fkv_io_sim -- --check` 通过。
5. `crate/io/fkv_io_sim/src/lib.rs` 不再是 1 行空壳；`tool/orphan_allow.tsv` 的
   `fkv_io_sim test-only` 白名单仍成立（无生产消费者，`./tool/orphan.sh` 未报过期）。

### 与方案的偏差

- 未加 `fkv_buf` 依赖：`FixedBuffer` 由 `fkv_io` 直接提供，无需直接依赖（避免 unused dep）。
- `delay_li` / `fault_li` 用 `OpKind` 而非 `Op` 做匹配键（`Op` 带具体参数，无法用作类别配置）。
- `op_log()` 返回 `Vec<OpRecord>`（`Mutex` 无法安全暴露 `&[Op]`）；`OpRecord` 携带 `clock`/`delay`。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_sim/src/lib.rs`（1289 行，非空壳）：`OpKind:40`、`Op:78`、
  `OpRecord:249`、`delay_li/fault_li/reorder_seed:265-267`、`SimStore:310`、
  `crash:378`、`restart:385`、`snapshot:405`（+ `snapshot_of:519`）。
- 六组 trait（PageIo/ManifestIo/TermIo/SegmentIo/FileIo/FixedSegmentIo）实现、
  durable/volatile 双层 + 删除墓碑语义与 md 一致。
- 测试 `tests/sim.rs`：`:13 crash_drops_unsynced_and_keeps_synced`、
  `:41 crash_drops_unsynced_manifest_and_delete_tombstone`、
  `:81 reorder_seed_is_deterministic`、`:100 fault_returns_error_and_keeps_state`、
  `:309 fault_at_zero_never_fires` 全部在位。
- `tool/orphan_allow.tsv:9` `fkv_io_sim test-only` 白名单仍成立。
