# redb 基准指标映射

redb 的基准代码在 `crates/redb-bench/src/lib.rs`，输出 `ResultType`：
`Throughput { count, duration, unit }`、`Latency(Duration)`、`SizeInBytes`、`NA`。
单位分为 `key/s`、`txn/s`、`scan/s`。

## 1. redb 测什么

| redb 阶段 | 指标 | 单位 |
|---|---|---|
| bulk load | 批量写入 | key/s |
| individual writes | 单条事务提交 | txn/s |
| small batch writes | 每事务 1000 条 | key/s |
| nosync writes | 关闭同步后的写入 | txn/s |
| len() | 读取 key 数量 | latency |
| random reads | 随机点读，3 次取中位数 | key/s |
| random range reads | 随机起点，每次读 10 条，3 次取中位数 | scan/s |
| random reads (4/8/16/32 threads) | 多线程点读 | key/s |
| removals | 随机删除一半 | key/s |
| retain | 按谓词保留一半 | key/s |
| extract_if | 区间内隔条删除 | key/s |
| pop | 从两端弹出 | key/s |
| sorted inserts | 有序批量插入 | key/s |
| uncompacted size | 压实前目录字节 | bytes |
| compacted size | 压实后目录字节 | bytes |

## 2. fkv / C++ 对应

| redb 指标 | fkv | C++ EloqStore | 状态 |
|---|---|---|---|
| 批量写入 key/s | `put_kv_s`、`put_mib_s` | `write summary` kvs/s | 已有 |
| 单条事务 txn/s | `--batch-size 1` 的 `put_batch_s` | `--batch_size 1` 的 write summary | 参数可模拟 |
| 小批写入 key/s | `put_kv_s` | `write summary` | 已有 |
| nosync writes | 无（fkv 每批 sync） | 无 | 不适用 |
| len() latency | 无公开 key count API | 无 | 未实现 |
| 随机点读 key/s | `get_kv_s`、`get_mib_s` | `read average` QPS | 已有 |
| 随机范围读 scan/s | `scan_kv_s`、`scan_page_s`、`scan_p99_us` | `scan throughput`、`scan speed` | 已有，用 `perf_local.sh --scan` |
| 多线程点读 | `read_per_part` 条读链 | `read_per_part` / `read_thds` | 参数已有；`scripts/perf_scale_read.sh` 扫 1/2/4/8 |
| removals | `--del-ratio` 混合删除 | 同一 Writer 混合 delete | 只有混合删除 |
| retain / extract_if / pop | 无对应 API | 无 | 未实现 |
| sorted inserts | `--workload load` 顺序写近似 | `--load=1` | 近似 |
| uncompacted size | `store_bytes` | `store` bytes | 已有 |
| compacted size | `compact_store_bytes_before/after`、`compact_reclaim_ratio` | C++ simple_bench 不触发 compact | 仅 Rust |
| 写放大 | `write_amp` = proc write / 逻辑写 | 无原生计数器 | Rust 已有 |
| 空间放大 | `space_amp_nominal` = store / 名义数据 | 无原生计数器 | Rust 已有 |
| reopen | `reopen_us` | 无 | Rust 已有 |
| 延迟尾部分位 | `*_p999_us` / `*_p9999_us` | write/read/scan p99.9 / p99.99 | 已有 |
| 延迟 p95 | `*_p95_us` | 无 | Rust 已有 |
| store 体积分解 / cache | `store_bytes_*`、`cache_*` | 无 | 仅 Rust |

## 3. 结论

redb 的点读、范围读、多线程读、目录大小和压缩前后大小，已经能在 fkv 侧找到
对应指标；C++ `simple_bench` 能对上的只有 put/get/scan 吞吐与延迟、目录大小
和 RSS。redb 的 `retain` / `extract_if` / `pop` / `len()` / `savepoint` /
`nosync` 在 fkv 和 C++ simple_bench 都没有同口径 API，不能为了凑指标伪造。

新增的 `scripts/perf_local.sh --scan` 会跑 C++ `ScanLoop` 和 Rust `scan_page`
的随机范围读，产出 `scan_kv_s`、`scan_page_s`、`scan_p99_us` 及
`scan_kv_s/cpp` 比值。
