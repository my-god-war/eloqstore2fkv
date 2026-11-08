# redb 评测框架参考

日期：2026-11-06

本文只讨论 **redb 的评测框架**，不把 redb 的绝对吞吐当成 fkv / C++ 的对比
基线。redb 的 5M 条 / 4 GiB cache / 单事务 bulk load 与 fkv 的 1M 条 /
O_DIRECT / 每批 sync 不是同一口径，直接比值会误导。

参考实现：`/tmp/redb/crates/redb-bench/src/lib.rs`（redb 4.5.0）。

## 1. redb 框架里值得吸收的部分

| 框架能力 | redb 做法 | fkv 是否已有 | 本次动作 |
|---|---|---|---|
| 固定阶段序列 | bulk load → individual writes → batch writes → reads → range reads → removals → size | 部分：`write-read` / `write-only` / `read-only` / `scan` | 保留现有 workload，增加阶段级 trial |
| 阶段级重复 | 读 / scan 各跑 3 次，取阶段耗时中位数 | `--repeat` 重复整个 workload（含写） | 新增 `--trials N`，同一 store 写一次、读 / scan N 次取中位数 |
| 结果类型 | `ResultType::{Throughput, Latency, SizeInBytes, NA}` | 指标是 `(name, value, unit)`；gate 按后缀判方向 | 不重构 JSON，补 unit / info 分类 |
| 不支持项显式 N/A | `compact()` / `set_sync()` 不支持时输出 `N/A` | 没有同口径 API 的指标直接不输出 | 保留缺失，不造 0 |
| 多引擎表 | 同一 `benchmark()` 跑多个 adapter，逐行高亮最优 | `fkv_perf_cross` + `perf_gate compare` | 新增 `scripts/perf_table.py` 渲染 |
| 并发读扫描 | 4 / 8 / 16 / 32 线程各一行 | `--read-per-part` / `--partitions` 可配，但没有 sweep | 新增 `scripts/perf_scale_read.sh` |
| compact 前后大小 | `uncompacted size` / `compacted size` 两行 | `compact` lane 只有文件数 / 吞吐 | 新增 `compact_store_bytes_before/after`、`compact_reclaim_ratio` |
| 分位 / 延迟 | 只有阶段耗时，没有每请求分位 | 已有 p50/p90/p99/p999/p9999/max | 补 p95 |
| 体积 / 文件数 | 递归目录求和 | 只有 `store_bytes` | 补 data / segment / manifest / other 分解与文件数 |
| cache 统计 | 库有 `cache_stats()`，bench 没输出 | `Store::index_cache_stat()` 已公开，bench 未调用 | 补 `cache_*` |
| 恢复 / reopen | 没有 | `reopen_us` | 已有，保持 |
| 写放大 / 空间放大 | 没有 | `write_amp` / `space_amp_nominal` | 已有，保持 |
| 每请求吞吐 / 带宽 | DB benchmark 不报 MB/s | `{put,get,scan,fullscan}_mib_s` | 已有，保持 |
| 逐秒时间序列 | 没有 | `*_window_*` | 已有，保持 |

## 2. 本次落到框架里的改动

### 2.1 `fkv_bench --trials N`

redb 对随机读 / 范围读各做 3 次，取阶段耗时中位数。fkv 原来的 `--repeat`
重复整个 workload（每轮重建 store、重写数据），测的是跨进程离散度，不是
「同一份数据上的稳态读」。

`--trials N`：

- 写阶段只跑一次；
- `reopen` 一次，丢掉 index cache；
- 读 / scan 阶段在同一个 store 上跑 N 次；
- 每个指标取 N 次的中位数，输出仍然只有一份；
- 默认 1，保持旧行为。

C++ `simple_bench` 没有阶段级 trial，`perf_local.sh --trials > 1` 会警告两侧
口径不对称，但继续跑；C++ 侧只能用 `--runs N` 做整轮重复。

### 2.2 `scripts/perf_scale_read.sh`

redb 的 `random reads (4/8/16/32 threads)` 是独立阶段。fkv / C++ 的
`read_per_part` 已经能配并发，但脚本没有 sweep。

新增脚本：

- 默认 `--readers 1,2,4,8`；
- Rust 先 `--load-only` 一次，再对每个并发档 `--read-only-existing`；
- C++ 先 `--workload write` 一次，再对每个并发档 `--workload read`；
- 输出 `impl / readers / qps / p99_us / speedup`，TSV 或 JSON；
- C++ 若不能重开已有 store，用 `--cpp-flow fresh`，明确标记为不同 flow。

### 2.3 延迟 p95

原分位是 p50/p90/p99/p999/p9999/max。补 `p95_us`，覆盖常见的 YCSB / RocksDB
报告口径；窗口指标仍只保留 p50/p90/p99，避免每秒窗口膨胀。

### 2.4 store 体积分解与 cache 统计

redb 只报总目录大小。fkv 侧补：

```text
store_bytes_data
store_bytes_segment
store_bytes_manifest
store_bytes_other
store_file_num
store_file_num_data
store_file_num_segment
store_file_num_manifest
cache_hit_num
cache_miss_num
cache_evict_num
cache_bytes
cache_len
cache_capacity
cache_hit_ratio
```

`cache_*` 来自 `Store::index_cache_stat()`，是 index cache 的累计命中 / 淘汰
计数与当前占用；没有 data cache，不能把 `cache_hit_ratio` 当成 data page
命中率。

### 2.5 compact 前后体积

`compact` lane 在准备阶段写完后记 `compact_store_bytes_before`，压力跑完后记
`compact_store_bytes_after`，并给出：

```text
compact_reclaim_ratio = 1 - after / before
```

这是 redb `uncompacted size` / `compacted size` 的同类指标，但 fkv 的 lane 是
「碎片写 + 并发 compact + 并发读」，比 redb 的结束一次性 `db.compact()` 更
接近线上维护路径。

## 3. 不照搬的部分

| redb 项 | 不照搬的原因 |
|---|---|
| `retain` / `extract_if` / `pop` | fkv `Store` 没有 predicate / pop API；为了凑指标新增 API 会改变引擎面 |
| `len()` | fkv 没有 O(1) key count；fullscan 计数不是同一语义 |
| `nosync writes` | fkv 每批 sync；去掉 sync 会改变持久化语义 |
| 4 GiB cache / 5M 条 | 与 fkv / C++ 基线不是同一缓存 / 数据量口径 |
| 绝对吞吐对比 | 缓存命中、硬件、持久化粒度全不同；只能作为 workload 覆盖参考 |
| 单一 `benchmark()` 泛型重构 | fkv 的指标模型已经比 redb 宽（分位、带宽、写放大、窗口、IO 计数）；重构收益低、风险高 |

## 4. 同机 fkv / C++ 基线仍然单独看

redb 不参与 fkv vs C++ 的比值。现有同机基线仍是：

| 指标 | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Put 吞吐 | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get 吞吐 | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| 峰值 RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| 存储占用 | 269.0 MiB | 101.4 MiB | 0.38x |

来源：`perf-baseline` schema 2，2 vCPU，4 KiB 页，O_DIRECT，1M 条、128B、
10,000 批、每批 100 条。C++ 存储含未清理历史页，Rust 存储是退出前采样值，
两者都不是「存活数据空间放大」。

## 5. 仍缺的框架能力

1. **delete-only phase**：redb 的 `removals` 是纯删除吞吐；fkv 只有混合
   `--del-ratio`。需要新增 `delete-only` lane。
2. **普通 workload 的 end-of-run compact**：redb 在每个 benchmark 末尾
   `db.compact()` 并报前后大小；fkv 只有 `compact` 诊断 lane。
3. **多表 / 多线程写 sweep**：redb 有 1..32 线程的独立表装载；fkv 的
   `--partitions` 是分片，不是每线程独立表。
4. **savepoint / archive lane**：fkv 有 archive API，尚未接入 bench。
5. **C++ 侧字节计数**：逻辑带宽和写放大目前只有 Rust 侧完整；C++ 仍靠 QPS
   事后换算或进程级 `/proc` 近似。
