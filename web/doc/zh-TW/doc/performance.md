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

## 目前結果

2026-11-06T04:25Z 的 schema 2 基線，`github-ubuntu-latest-x86_64`、4 KiB 頁、
`data_append_mode=false`、`direct_io=true`。固定 workload：
`rows/max_key=1000000`、`batch_size=100`、`write_batchs=10000`、
`read_per_part=4`、`partitions=1`、`kv_size=128`、`rng=cpp`
（`key_interval=4` / `del_ratio=4`）、`test_secs=30`。

| 專案 | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Put 吞吐 | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get 吞吐 | 5,526 QPS | 7,293 QPS | 1.32x |
| Put 邏輯頻寬 | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Get 邏輯頻寬 | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| 峰值 RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| 儲存佔用 | 269.0 MiB | 101.4 MiB | 0.38x |
| 儲存 / 名義數據 | 2.20x | 0.83x | 0.38x |

寫入 100 萬條（10,000 批）對應
`io_write_at_num=48,625`、`io_data_sync_num=10,000`，即每批一次資料同步。
Rust 額外指標：Scan `141,166 kv/s`、Fullscan `108,274 kv/s`、寫放大 `2.57x`
（`proc_io_write` 329 MB / 128 MB 邏輯寫）、每批 `4.86` 次寫系統呼叫。
儲存倍率以 1M × 128B = 128 MB 名目資料計算；C++ 這次沒有觸發 GC / compact，
所以它包含更多歷史頁。它不是「存活資料空間放大」的同口徑數字。
Rust 的 P99.9 尾延遲仍比 C++ 差（put 223.3 ms vs 86.0 ms，get 355.5 ms vs
48.2 ms），單基線波動較大，詳見內部 perf report。

**說明**：C++ 的 `write-read` 是寫執行緒與讀取執行緒同時運行，Rust 目前是
「寫完、reopen、再讀」，讀階段沒有寫競爭。因此 `write-read` 的倍率不能當作
純讀 / 純寫結論；嚴格對比應看 `scripts/perf_local.sh --symmetric`
產出的 write-only 與 read-only 兩對結果。單次分享 runner 數字只用於迴歸
趨勢，發布判斷使用 `--runs` 的中位數。