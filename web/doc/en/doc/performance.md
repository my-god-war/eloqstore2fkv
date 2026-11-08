# Performance

Download `perf-latest` of `fkv_bench` from the fkv repository and run the same workload on a fixed machine.
And compare throughput, latency quantiles, and peak RSS to a C++ baseline.

The parameter source is consistent with fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

The C++ baseline is run by the oracle environment `simple_bench` and then output according to the same schema and published to
`perf-baseline` release asset. The comparison results are written back to the warehouse document, and the site directly renders Markdown.
No JSON data layer is maintained.

## Current results

schema 2 baseline for 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB page,
`data_append_mode=false`, `direct_io=true`. Fixed workload:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| Project | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Put throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get throughput | 5,526 QPS | 7,293 QPS | 1.32x |
| Put logical bandwidth | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Get logical bandwidth | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Storage usage | 269.0 MiB | 101.4 MiB | 0.38x |
| Storage / nominal data | 2.20x | 0.83x | 0.38x |

Writing 1 million entries (10,000 batches) corresponds to
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, that is, data is synchronized once per batch.
Rust additional metrics: Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, Write Amplification `2.57x`
(`proc_io_write` 329 MB / 128 MB logical writes), `4.86` write system calls per batch.
The storage ratio is calculated based on 1M × 128B = 128 MB nominal data; C++ does not trigger GC/compact this time,
So it contains more pages of history. It is not the same caliber number as "survival data space amplification".
Rust's P99.9 tail latency is still worse than C++ (put 223.3 ms vs 86.0 ms, get 355.5 ms vs
48.2 ms), the single baseline fluctuates greatly, please see the internal perf report for details.

**Note**: C++'s `write-read` means that the writing thread and the reading thread run at the same time, and Rust currently
"Complete writing, reopen, and read again", there is no writing competition in the reading stage. Therefore the multiplier of `write-read` cannot be treated as
Pure reading / pure writing conclusion; strict comparison should see `scripts/perf_local.sh --symmetric`
Two pairs of results are output: write-only and read-only. Single shared runner numbers are only used for regression
Trends, releases are judged using the median of `--runs`.