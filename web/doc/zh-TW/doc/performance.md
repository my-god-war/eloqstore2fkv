# 效能

從 fkv 倉下載 `perf-latest` 的 `fkv_bench`，在固定機器上跑同一支 workload，
並與 C++ 基線比較吞吐、延遲分位與峰值 RSS。

參數來源與 fkv 保持一致：

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ 基線由 oracle 環境跑 `simple_bench` 後以相同 schema 產出，發佈到
`perf-baseline` release asset。比較結果寫回本倉文檔，站點直接渲染 Markdown，
不維護 JSON 資料層。