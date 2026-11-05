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