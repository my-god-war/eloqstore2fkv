# Pagganap

I-download ang `perf-latest` ng `fkv_bench` mula sa fkv repository at patakbuhin ang parehong workload sa isang nakapirming machine.
At ihambing ang throughput, latency quantile, at peak RSS sa isang C++ baseline.

Ang pinagmulan ng parameter ay pare-pareho sa fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Ang C++ baseline ay pinapatakbo ng oracle environment `simple_bench` at pagkatapos ay i-output ayon sa parehong schema at nai-publish sa
`perf-baseline` release asset. Ang mga resulta ng paghahambing ay isinulat pabalik sa dokumento ng warehouse, at ang site ay direktang nag-render ng Markdown.
Walang pinapanatili na layer ng data ng JSON.