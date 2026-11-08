# redb 风格性能矩阵

日期：2026-11-06

目标不是复刻 redb 的绝对数字，而是把 redb crates.io 页面的阶段形状搬到
C++ EloqStore 与 Rust fkv 的同机对比里：

```text
bulk load
individual writes
batch writes
len()
random reads
random range reads
random reads (1/2/4/8 threads)
removals
uncompacted size
compacted size
```

## 1. 统一入口

```bash
scripts/perf_redb_matrix.sh \
  --rows 1000000 \
  --batch-size 1000 \
  --kv-size 128 \
  --read-secs 10 \
  --readers 1,4,8,16,32 \
  --workdir /tmp/fkv-matrix \
  --markdown /tmp/fkv-matrix.md \
  --json /tmp/fkv-matrix.json
```

默认复用已构建二进制。首次或换机器：

```bash
scripts/perf_redb_matrix.sh --rebuild ...
```

`--rows` 是数据集 key 数；`--batch-size` 是 batch writes 的批大小，
individual writes 固定 batch=1。

也可以直接给二进制传 C++ FFI 共享库：

```bash
cargo run -p fkv_bench_matrix -- \
  --cpp-lib /path/to/libfkv_bench_ffi.dylib \
  --rust-bin target/debug/fkv_bench \
  --rows 1000000 --batch-size 1000 --kv-size 128 \
  --read-secs 10 --readers 1,2,4,8 \
  --workdir /tmp/fkv-matrix
```

## 2. trait 结构

Rust 侧定义：

```rust
trait BenchEngine {
    fn name(&self) -> &'static str;
    fn run(&self, phase: Phase, cfg: &MatrixConfig) -> Result<PhaseResult, String>;
    fn store_bytes(&self, cfg: &MatrixConfig) -> Result<u64, String>;
    fn compact(&self, cfg: &MatrixConfig) -> Result<Option<Duration>, String>;
    fn supports(&self, phase: Phase) -> bool;
}
```

实现：

- `CppFfiEngine`：FFI 适配器，首选。运行时 `dlopen` `--cpp-lib` 指向的
  共享库，解析 `fkv_bench_open/close/run/last_error`，在矩阵进程内跑 C++
  引擎；全部 phase 可用（`len` 是全表扫描计数）。
- `CppEngine`：legacy 进程适配器，`--cpp-bin` fallback，调用
  `simple_bench`，解析 `write summary` / `read average` /
  `scan throughput` / `compact total` 日志。
- `FkvEngine`：进程适配器，调用 `fkv_bench --history`，解析 JSON。

构建期不链接 C++：`cargo build` / `cargo test` 都不需要 `.so`，只有运行时
带 `--cpp-lib` 才 `dlopen`。若同时提供 `--cpp-lib` 与 `--cpp-bin`，
`--cpp-lib` 优先；只给 `--cpp-bin` 时退回进程适配器。FFI 路径下 C++ 引擎
仍依赖 bthread / brpc / io_uring 与 Rust compio runtime 同进程共存，线程、
信号和 io_uring 生命周期由 shim 内部隔离；进程适配器保留给旧二进制和排障。
两个都不给时 CLI 直接报错，不会静默跳过 C++ 列。

## 3. C++ FFI ABI

`--cpp-lib` 指向的共享库导出固定 C ABI：

```c
typedef struct FkvBenchFfi FkvBenchFfi;
typedef struct { uint64_t ops; double secs; double p99_us; } FkvBenchResult;

FkvBenchFfi *fkv_bench_open(const char *root, uint32_t rows,
                            uint32_t batch_size, uint32_t kv_size,
                            uint32_t partitions);
void          fkv_bench_close(FkvBenchFfi *h);
int           fkv_bench_run(FkvBenchFfi *h, const char *phase,
                            uint32_t readers, double read_secs,
                            FkvBenchResult *out);
const char   *fkv_bench_last_error(FkvBenchFfi *h);
```

Rust 侧用 `libc::{dlopen,dlsym,dlclose}` + `RTLD_NOW` 载入；四个符号缺一
即启动失败。矩阵行为：

- `run`：删除 `workdir/cpp-ffi/<phase>`，以 `partitions=1` 调
  `fkv_bench_open(root, rows, batch_size, kv_size, 1)`，再按 phase 名调
  `fkv_bench_run`。
- `bulk_load` 的 handle 保留给随后的 `compact` 复用；其它 phase 跑完立即
  close。每次新 `run` 开始前先关掉残留 handle，避免和刚被删除的目录共用
  句柄。
- `compact`：对保留的 handle 调 `phase="compact"`，用结果里的 `secs` 作为
  压实耗时，然后 close。
- `store_bytes`：对最近一次 `run` 的 root 递归求和目录字节。
- 错误：`fkv_bench_run` 返回非 0 时读取 `fkv_bench_last_error` 并附在错误
  文本中；`fkv_bench_open` 返回 NULL 同样报错。
- `readers` / `read_secs`：`random_reads` 传真实 readers 与 `--read-secs`，
  `range_reads` 传 `readers=1` 与 `--read-secs`，写/删除/compact/len 传
  `readers=1, read_secs=0`。
- 结果映射：`ops` 原样保留，`secs` 变成 duration，吞吐为 `ops / secs`；
  `p99_us > 0` 时写入表格，否则留空。

phase 名映射：

| Phase | `fkv_bench_run(phase)` |
|---|---|
| BulkLoad | `bulk_load` |
| IndividualWrites | `individual_writes` |
| BatchWrites | `batch_writes` |
| RandomReads | `random_reads` |
| RangeReads | `range_reads` |
| Removals | `removals` |
| Len | `len` |

共享库文件名随平台：Linux `libfkv_bench_ffi.so`，macOS
`libfkv_bench_ffi.dylib`。`cargo test` 不加载它；缺失时只有真正带
`--cpp-lib` 运行才会报 `dlopen ... failed`。

## 4. 每个 phase 怎么跑

| phase | C++ FFI | fkv |
|---|---|---|
| bulk load | `fkv_bench_run("bulk_load")` | `--load-only --workload write-only` |
| individual writes | `fkv_bench_run("individual_writes")` | `--workload write-only --batch-size 1 --write-batchs R` |
| batch writes | `fkv_bench_run("batch_writes")` | `--workload write-only --batch-size B --write-batchs R/B` |
| random reads | `fkv_bench_run("random_reads", readers=N)` | 先 `--load-only`，再 `--read-only-existing --workload read-only --read-per-part N` |
| random range reads | `fkv_bench_run("range_reads")` | 先 `--load-only`，再 `--read-only-existing --workload scan` |
| removals | `fkv_bench_run("removals")` | `--workload delete-only`（内部先不计量灌数据，再计时删除） |
| len() | `fkv_bench_run("len")`（全表扫描计数） | `--workload scan --test-secs 0` 单轮全表扫描计数 |
| uncompacted size | bulk load 后目录字节 | `--load-only` 后目录字节 |
| compacted size | `fkv_bench_run("compact")` 后目录字节 | `--read-only-existing --workload compact-only` 后目录字节 |
| compact wall time | `FkvBenchResult.secs` | fkv `compact_total_us` |

legacy `--cpp-bin` 走同一张 workload 表，但通过 `simple_bench` 命令行参数
表达（见上一节）。`len()` 在 C++ FFI 与 fkv 两侧都是全表扫描计数，不是
O(1) key count；表格脚注会明确标注 `scan-based`。

## 5. 压实前后

矩阵固定跑：

```text
bulk load -> 测 uncompacted size -> compact -> 测 compacted size
```

C++ 侧用 `CompactRequest` 逐分区同步 compact；fkv 侧用
`compact_if_needed` 逐分区 compact。表格输出：

```text
| uncompacted size | ... |
| compacted size   | ... |
| compact wall time | ... |
```

注意：

- C++ pooled 模式的 data compaction 只在 append 模式下生效；pooled 下
  `CompactRequest` 主要做 FileGc，compacted size 可能等于 uncompacted size。
- fkv `compact-only` 在尾文件未满时也可能 `compact_file_num = 0`。这不是
  指标缺失，是 compact_plan 有意跳过尾文件。
- 因此 `compacted size` 必须和引擎的分配模式一起读，不能只看数字。

## 6. 表格输出

```text
| phase | cpp | fkv | ratio |
| bulk load | 293.0 kops/s | ... | 1.27x |
| individual writes | ... | ... | ... |
| batch writes | ... | ... | ... |
| random reads (1 thread) | ... | ... | ... |
| random range reads | ... | ... | ... |
| removals | ... | ... | ... |
| len() | scan count | scan count | 1.0x |
| uncompacted size | 4.00 GiB | ... | ... |
| compacted size | 1.69 GiB | ... | ... |
| compact wall time | ... | ... | ... |
```

`ratio = fkv / cpp`，大于 1 表示 fkv 更快。size ratio 是 fkv / cpp 字节比。

## 7. C++ FFI 构建与 patch

FFI 共享库：

```bash
scripts/ci/build_cpp_ffi.sh
# 最后一行输出 libfkv_bench_ffi.so / .dylib
```

它编译 `cpp/bench_ffi`，链接 pinned eloqstore。Rust 侧不会在 `cargo build`
阶段链接 C++，只通过 `--cpp-lib` 在运行时 `dlopen`。

`cpp/eloqstore` 是 fetch 下来的固定 commit，不在主仓跟踪。`simple_bench` 的
新 phase 通过：

```text
cpp/patches/simple_bench_phases.patch
```

维护。`scripts/ci/build_cpp_bench.sh` 在 fetch 后幂等应用：

```text
git -C cpp/eloqstore apply --check ../../cpp/patches/simple_bench_phases.patch
```

已应用时用 `--reverse --check` 跳过。升级 eloqstore pin 后如果 patch 不再
适用，构建脚本会直接失败，不会静默丢 phase。

## 8. 当前限制

1. C++ 首选同进程 FFI（`--cpp-lib`）；legacy `--cpp-bin` 进程适配器仅作
   回退，两者不能在同一个矩阵进程里混用。
2. `len()` 两侧都是全表扫描计数，不是 O(1)；C++ 走 FFI `len` phase，Rust 走
   `fkv_bench --workload scan --test-secs 0` 的单轮 `fullscan_num` /
   `fullscan_total_us`。
3. C++ `read` 的 p99 取周期窗口最大值；Rust 取整个时间窗。口径差异保留在
   表格脚注里。
4. C++ `delete` 和 `compact` 依赖 patch；旧二进制会被矩阵的 flag 预检拒绝。
5. compaction 对 pooled / append 的行为不同，`compacted size` 不能脱离分配
   模式解读。

## 9. redb 口径跑通（run 42397058871）

CI 上按 `crates/redb-bench/src/lib.rs` 的形状跑完整矩阵，`perf-gate` 与
`perf-matrix` 全绿，耗时约 22 分钟。

| phase | cpp | fkv | ratio |
|---|---:|---:|---:|
| bulk load | 206.95 kops/s | 859.83 kops/s | 4.155x |
| individual writes | 1.35 kops/s | 2.59 kops/s | 1.919x |
| batch writes | 112.57 kops/s | 881.42 kops/s | 7.830x |
| random reads 1T | 450.33 kops/s | 636.93 kops/s | 1.414x |
| random reads 4T | 378.48 kops/s | 660.27 kops/s | 1.745x |
| random reads 8T | 374.71 kops/s | 665.25 kops/s | 1.775x |
| random reads 16T | 384.09 kops/s | 663.43 kops/s | 1.727x |
| random reads 32T | 375.94 kops/s | 666.43 kops/s | 1.773x |
| random range reads | 4.96 kops/s | 123.88 kops/s | 24.974x |
| removals | 1.55 Mops/s | 1.78 Mops/s | 1.148x |
| len() | 579.55 kops/s | 28.82 Mops/s | 49.720x |
| sizes | 787.57 MiB | 787.57 MiB | 1.000x |

### 9.1 之前 47 分钟"卡住"的真实原因

run 42389635181 在第 8 步挂了 47 分钟没有结束。取消后从日志拿到分阶段墙钟：

```
bulk load          16:23:12 → 16:24:08    56 s
individual writes                          1.4 s
batch writes                               0.7 s
random reads 1T    16:24:10 → 16:43:05    18 min 55 s   ← ~2.6k ops/s
random reads 4T                            5 min 15 s
random reads 8T                            4 min 28 s
random reads 16T                           4 min 13 s
random reads 32T                           4 min 30 s
range reads        17:01:31 → 取消         > 8 min
```

不是 32 读线程（读次数按 `base + remainder` 均分，代码正确），而是**缓存
预算和 redb 口径不一致**：redb 的 `CACHE_SIZE = 4 GiB` 大于它 870 MB 的数
据集，读阶段是全驻内存测量；当时矩阵用 64 MiB < 174 MB 数据集，两侧随机
读全落慢盘，1 线程 1M 次点读要 19 分钟，测的是盘不是引擎。

改法：缓存两侧 1 GiB（仍大于 166 MiB 数据集，同"全驻内存"条件）。改完点读
从 2.6 kops/s 升到 450 kops/s，整轮 22 分钟。

### 9.2 两处需要读者注意的口径

- **random range reads 25x**：C++ 侧每次 `ScanRequest` 有 ~200 µs 的固定
  开销（点读只要 2 µs，说明不是 ExecSync 往返的问题，是 scan 路径本身），
  与窗口大小无关；fkv 侧固定开销极低。上一轮 200k 行、每次 256 条的矩阵里
  同一对引擎是 0.799x，说明两者是「固定开销 vs 每条成本」的交叉，不是单方
  面实现优劣。这一行随窗口大小变化剧烈，引用时必须带上 `scan-len`。
- **len() 49.7x**：两侧都没有 O(1) 计数接口。C++ `RunLen` 走分页 scan，每次
  物化 256 条再计数（~440 µs/页）；fkv 的 `scan_count` 只计数不物化。本地
  复核 `fullscan_kv_s = 31.7M`（1M 键 31.5 ms，与 CI 的 28.8M 同量级），确认
  不是解析错误。**不能**与 redb README 的 `len()` 比较，redb 那是 O(1)。

### 9.3 新增的防护

- `--phase-timeout-secs`（默认 1800，CI 传 900）：外部进程超时即杀并报错，
  日志照常落盘。配 1 例真实超时测试（`/bin/sleep 5` + 300ms 上限必须被杀）。
- 矩阵 artifact 同时上传 `*/logs/**`：失败时不用重跑也能看到卡在哪个阶段。
- `num_threads` 固定 1：它是 EloqStore 的 shard 数，单分区起 32 个 shard 会
  让 io_uring 初始化 ENOMEM（run 42385622423 的 32 线程阶段）。

## 10. 口径改为 RocksDB 官方评测（run 42431241655）

redb 的发布条件是「缓存装得下数据集」（4 GiB 缓存 / 870 MB 数据），读全命中内存；
RocksDB 的 `PERFORMANCE-BENCHMARK` 相反：`NUM_KEYS=900M`、`CACHE_SIZE=6 GiB`，
数据库 ≈ 97 GiB，**数据 / 缓存 ≈ 16:1**，读真的走盘。压测数据库时后者才是真实
工况，因此首页口径改用 RocksDB：

| 项 | RocksDB 官方 | 本矩阵 |
|---|---|---|
| key / value | db_bench 默认 16 B / 100 B | 同（kv_size 116） |
| 数据 | 97 GiB | 25M 行 ≈ 2.77 GiB |
| 缓存 | 6 GiB | 256 MiB |
| 数据:缓存 | ≈16:1 | ≈11:1 |
| 并发 | `benchmark.sh` 默认 16 线程 | 16 线程 |
| 测试 | readrandom | random reads，1 次采样 |

结果：

| 阶段 | EloqStore (C++) | fkv | C++ / fkv |
|---|---:|---:|---:|
| random reads (16 threads) | **46887ms** | 55070ms | 0.851x |
| uncompacted size | 2.59 GiB | 2.59 GiB | 1.001x |

即 1M 次随机点读、数据约 11 倍于缓存时：C++ 21.3k ops/s，fkv 18.2k ops/s，
**fkv 慢 15%**。同一个引擎在「数据 < 缓存」时 fkv 曾领先 1.75x（§9），说明优势
来自驻留内存下的读路径，而不是磁盘路径；把数据放到缓存装不下后差距翻转为
C++ 略优。

一个反面教材记在这里：最初把 25M 行配 4 GiB 缓存（数据 ≈ 缓存）跑了 80 分钟
没结束——那是 LRU 的最坏点（每次访问都挤掉下一步要用的页），既不是「全驻内存」
也不是干净的「冷缓存」，数字重复性极差，已废弃该配置。
