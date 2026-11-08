# 性能

从 fkv 仓下载 `perf-latest` 的 `fkv_bench`，在固定机器上跑同一 workload，
并与 C++ 基线比较吞吐、延迟分位与峰值 RSS。

参数来源与 fkv 保持一致：

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ 基线由 oracle 环境跑 `simple_bench` 后按同一 schema 产出，发布到
`perf-baseline` release asset。对比结果写回本仓文档，站点直接渲染 Markdown，
不维护 JSON 数据层。

## 当前结果

2026-11-06T04:25Z 的 schema 2 基线，`github-ubuntu-latest-x86_64`、4 KiB 页、
`data_append_mode=false`、`direct_io=true`。固定 workload：
`rows/max_key=1000000`、`batch_size=100`、`write_batchs=10000`、
`read_per_part=4`、`partitions=1`、`kv_size=128`、`rng=cpp`
（`key_interval=4` / `del_ratio=4`）、`test_secs=30`。

| 项目 | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Put 吞吐 | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get 吞吐 | 5,526 QPS | 7,293 QPS | 1.32x |
| Put 逻辑带宽 | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Get 逻辑带宽 | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| 峰值 RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| 存储占用 | 269.0 MiB | 101.4 MiB | 0.38x |
| 存储 / 名义数据 | 2.20x | 0.83x | 0.38x |

写入 100 万条（10,000 批）对应
`io_write_at_num=48,625`、`io_data_sync_num=10,000`，即每批一次数据同步。
Rust 额外指标：Scan `141,166 kv/s`、Fullscan `108,274 kv/s`、写放大 `2.57x`
（`proc_io_write` 329 MB / 128 MB 逻辑写）、每批 `4.86` 次写系统调用。
存储倍率按 1M × 128B = 128 MB 名义数据计算；C++ 这次没有触发 GC / compact，
所以它包含更多历史页。它不是“存活数据空间放大”的同口径数字。
Rust 的 P99.9 尾延迟仍比 C++ 差（put 223.3 ms vs 86.0 ms，get 355.5 ms vs
48.2 ms），单次基线波动较大，详见内部 perf report。

**说明**：C++ 的 `write-read` 是写线程与读线程同时运行，Rust 当前是
「写完、reopen、再读」，读阶段没有写竞争。因此 `write-read` 的倍率不能当作
纯读 / 纯写结论；严格对比应看 `scripts/perf_local.sh --symmetric`
产出的 write-only 与 read-only 两对结果。单次共享 runner 数字只用于回归
趋势，发布判断使用 `--runs` 的中位数。
