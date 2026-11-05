# perf-71 bench get 全是热顺序读，未测冷点读，与验收口径不符

## 位置
- Rust: `crate/perf/fkv_bench/src/main.rs:326-333`（点读循环）、`main.rs:292`（key 编码）、`main.rs:301-315`（写阶段）
- C++: `benchmark/simple_bench.cpp:301-392`（`ReadLoop`）、`simple_bench.cpp:323`（随机 key）、`simple_bench.cpp:304`（并发读者数）

## 现状
fkv_bench 的点读阶段：
- key 编码为 `format!("key:{index:010}")`（`main.rs:292`），按 `index in 0..rows` **顺序**遍历（`main.rs:327`）。
- 写阶段刚把 `rows`（默认 20k）条数据全部写入（`main.rs:301-315`），紧接着读，全部命中 index cache / page cache，是**热顺序读**。
- `rows` 默认仅 20_000（`main.rs:93`），数据量远小于任何缓存预算，读必然全命中内存。

## C++ 实现 / 目标口径
C++ `simple_bench` ReadLoop：
- key 为 8 字节大端 uint64，按 `rand_gen() % FLAGS_max_key` **随机**取 key（`simple_bench.cpp:323`），`max_key` 默认 10_000_000。
- 读者数 = `read_per_part * partitions`（`simple_bench.cpp:304`），默认 `4*1=4` 个并发读者持续发请求。
- 随机 key 落在 10M 空间，冷读触发真实磁盘 IO。
- todo.md 验收标准第 6 条：**冷点读 p99 ≤ 1.2×**。

## 差异与风险
- fkv_bench 测的是「写后全热顺序读」，C++ 测的是「随机冷点读」。两者 IO 模式完全不同：前者 0 次磁盘 IO，后者每次读恰好 1 次磁盘 IO（验收目标 1）。
- 用热读数据去对标「冷点读 p99 ≤ 1.2×」验收项，**测量失真**：热读 p99 与磁盘延迟无关，无法证明冷读达标，也无法证伪。
- 顺序 key 还让 B+tree 查找退化为顺序叶扫描，掩盖了 B+tree 二分查找的真实开销。

## 对齐建议
- 点读 key 改为随机：用 `rand` 在 `[0, max_key)` 内取 key，对齐 C++ `rand_gen() % FLAGS_max_key`。
- key 编码对齐 C++：8 字节大端 uint64（`EncodeFixed64(ToBigEndian(key))`），而非 `key:{index:010}` 字符串。
- 读前可先 `store.reopen(tbl)` 或清缓存，确保测的是冷读；或在 read-only workload 下先写满 `max_key` 再随机读。
- `rows` 默认值应与 C++ `max_key` 对齐（10M 量级），否则数据规模不可比。

## 性能优化建议
- 冷读基准应在写满数据、缓存未覆盖随机区间后测量，才能暴露真实磁盘 IO 延迟。
- 顺序读可作为独立指标（warm scan）保留，但不得与 cold point-read 混用同一 metric 名。

## 严重度
P0（测量失真/验收无法达标）

## 审查结论（2026-11-04）

判定：**合理**，P0 测量失真成立，按建议修复。

核验结果：

- `crate/perf/fkv_bench/src/main.rs` 旧实现确为 `format!("key:{index:010}")` + `for index in 0..rows` 顺序读；写完立即读，20k 条数据全在 index cache / OS page cache，等于热顺序读。
- C++ 子模块为空（`cpp/eloqstore` 无源码），无法逐行对照；按 issue 引述与 `doc/migration.md` V6 口径（同机 NVMe 对比 `simple_bench`）确认目标为「随机 key 冷点读」，`max_key` 默认 10M。
- `Store::reopen` 在 `install_replay_state` 里重建 `IndexCache::default()`，可用于清 index 缓存，制造冷 index 读取路径。
- 顺序读价值保留：`scan_kv_s` / `scan_page_s` 已是独立指标，天然区分 warm scan，无需再拆 metric 名。

采纳项：

1. key 改为 8 字节大端 u64（`key_bytes`），对齐 C++ `EncodeFixed64(ToBigEndian(key))`；读路径栈上编码，零分配。
2. 点读改为在 `[0, rows)` 随机取 key（`fastrand` 固定 seed `READ_SEED`），seed 写入 `PerfData.read_seed` 保证可复现。
3. 读前对每张表 `reopen`，丢弃 index cache；计时从 reopen 之后开始。
4. `--rows` 默认值由 20_000 改为 `--max-key`（默认 10M），使默认数据规模与 key 空间对齐 C++；`perf.yml` 显式传 `--rows 20000`，CI 行为不变。

不采纳 / 拆出：

- `read_per_part` 的并发读者语义属 perf-73；延迟分位属 perf-72；本次只改读模式与 key 编码。

遗留：

- OS page cache 不可控。Linux 可再加 `CompioStore::with_direct_io(true)` 实现真数据页冷读，但会改变写路径对齐约束，本次未接；macOS 无 O_DIRECT。
- 默认 10M key × 1KiB ≈ 10GiB，需在 NVMe 上跑，本地验证请显式 `--rows` 调小。

## 实施方案

### 位置

- `crate/perf/fkv_bench/src/main.rs`：`key_bytes`、写阶段 batch 构造、点读循环、`--rows` 默认值、`PerfData`。
- `crate/perf/fkv_bench/Cargo.toml`：新增 `fastrand`。

### 方案

1. `const KEY_BYTES: usize = size_of::<u64>()`；`fn key_bytes(index: usize) -> [u8; KEY_BYTES]` 返回 `(index as u64).to_be_bytes()`。
2. 写阶段 `BatchEntry::upsert(key_bytes(index).to_vec(), ...)`。
3. 读前 `for (tbl, store) in &mut store_li { store.reopen(tbl).await.unwrap(); }`。
4. `let mut rng = fastrand::Rng::with_seed(READ_SEED);`，每轮 `let index = rng.usize(0..rows);`，按 `index % partitions` 选表。
5. `--rows` 未指定时默认取 `max_key`；`effective_rows` 上限逻辑保留。

### 验收

- `get_kv_s` 为随机 key 读，读前 index cache 已清空。
- 默认参数下读 key 空间 = `max_key` = 10M，与 C++ 对齐。
- `read_seed` 出现在 perf JSON，固定值可复现。
- `cargo nextest run -p fkv_bench` 全绿；`cargo clippy -p fkv_bench --all-targets --all-features --no-deps -- -D warnings` 零警告。

## DONE（2026-11-04）

### 改动

- `crate/perf/fkv_bench/src/main.rs`
  - 新增 `KEY_BYTES` / `READ_SEED` 常量与 `key_bytes(index) -> [u8; 8]`（`to_be_bytes`）。
  - 写阶段改用 `key_bytes(index).to_vec()`，删除 `format!` 拼 key。
  - 读前逐表 `store.reopen(tbl)` 清空 index cache；点读改为 `fastrand::Rng::with_seed(READ_SEED)` 在 `[0, rows)` 随机取 index，读路径零分配。
  - `--rows` 默认值改为 `--max-key`（10M），帮助文本同步。
  - `PerfData` 新增 `read_seed`（`#[serde(default)]`，兼容旧 history）。
  - 新增测试 `key_bytes_are_big_endian_and_ordered`。
- `crate/perf/fkv_bench/Cargo.toml`：`cargo add fastrand`（2.5.0）。

### 验证

- `cargo check -p fkv_bench --all-targets` → 通过（期间 `fkv_store` 被并发改动短暂编不过，等待后重试通过）。
- `cargo nextest run -p fkv_bench --no-fail-fast` → 6 passed / 0 failed。
- `cargo clippy -p fkv_bench --no-deps --all-features --all-targets -- -D warnings` → 0 警告。
- `cargo +nightly fmt -p fkv_bench -- --check` → clean。
- 冒烟：`cargo run -p fkv_bench -- --rows 2000 --max-key 2000 --read-per-part 2` 输出 5 个指标，JSON 含 `read_seed`。

### 遗留

- 数据页真冷读需 Linux `CompioStore::with_direct_io(true)`，本次未接（写路径对齐约束需单独验证），记为后续项。
- 默认 10M key 约 10GiB 数据，CI 仍用 `--rows 20000`，本地默认跑需 NVMe 与大磁盘。

## 复审（2026-11-05 r6）

通过。

- `fkv_bench/src/main.rs:53 KEY_BYTES`、`:55 READ_SEED`、`:585 key_bytes`
  （`to_be_bytes` 大端 u64）；写阶段用 `key_bytes(index).to_vec()`。
- `:752-754` 读前逐表 `store.reopen(tbl)` 丢弃 index cache（注释写明冷路径）；
  点读 `fastrand::Rng`（`:43`、`:654`）随机取 index。
- `:278-287` `--rows` 默认取 `--max-key`（10M），`:346 data.read_seed = READ_SEED`
  进 JSON；`:580 effective_rows` 受 max_key 约束。
- 测试 `:1210 key_bytes_are_big_endian_and_ordered`。
