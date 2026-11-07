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

## Mga kasalukuyang resulta

schema 2 baseline para sa 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB page,
`data_append_mode=false`, `direct_io=true`. Nakapirming workload:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proyekto | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Ilagay ang throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Kumuha ng throughput | 5,526 QPS | 7,293 QPS | 1.32x |
| Maglagay ng lohikal na bandwidth | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Kumuha ng lohikal na bandwidth | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Paggamit ng imbakan | 269.0 MiB | 101.4 MiB | 0.38x |
| Imbakan / nominal na data | 2.20x | 0.83x | 0.38x |

Ang pagsulat ng 1 milyong entry (10,000 batch) ay tumutugma sa
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, ibig sabihin, ang data ay naka-synchronize nang isang beses bawat batch.
Mga karagdagang sukatan ng kalawang: I-scan ang `141,166 kv/s`, Fullscan `108,274 kv/s`, Sumulat ng Amplification `2.57x`
(`proc_io_write` 329 MB / 128 MB logical writes), `4.86` magsulat ng mga system call bawat batch.
Ang storage ratio ay kinakalkula batay sa 1M × 128B = 128 MB nominal na data; Ang C++ ay hindi nagti-trigger ng GC/compact sa oras na ito,
Kaya naglalaman ito ng higit pang mga pahina ng kasaysayan. Hindi ito ang parehong kalibre ng numero bilang "survival data space amplification".
Ang P99.9 tail latency ng Rust ay mas masahol pa kaysa sa C++ (maglagay ng 223.3 ms vs 86.0 ms, makakuha ng 355.5 ms vs
48.2 ms), ang nag-iisang baseline ay lubhang nagbabago, pakitingnan ang internal na perf report para sa mga detalye.

**Tandaan**: Nangangahulugan ang `write-read` ng C++ na ang thread ng pagsusulat at ang thread ng pagbabasa ay sabay na tumatakbo, at kasalukuyang si Rust
"Kumpletuhin ang pagsulat, buksan muli, at basahin muli", walang kompetisyon sa pagsulat sa yugto ng pagbasa. Samakatuwid ang multiplier ng `write-read` ay hindi maaaring ituring bilang
Purong pagbabasa / purong pagsulat na konklusyon; dapat makita ng mahigpit na paghahambing ang `scripts/perf_local.sh --symmetric`
Dalawang pares ng mga resulta ang output: write-only at read-only. Ginagamit lang ang solong shared runner number para sa regression
Ang mga trend, ang mga release ay hinuhusgahan gamit ang median ng `--runs`.