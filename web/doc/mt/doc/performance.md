# Prestazzjoni

Niżżel `perf-latest` ta' `fkv_bench` mir-repożitorju fkv u ħaddem l-istess xogħol fuq magna fissa.
U qabbel throughput, latency quantiles, u l-ogħla RSS għal linja bażi C++.

Is-sors tal-parametru huwa konsistenti ma' fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Il-linja bażi C++ titmexxa mill-ambjent oracle `simple_bench` u mbagħad toħroġ skont l-istess skema u ppubblikata lil
`perf-baseline` rilaxx assi. Ir-riżultati tat-tqabbil jinkitbu lura fid-dokument tal-maħżen, u s-sit jirrendi direttament Markdown.
L-ebda saff tad-dejta JSON ma jinżamm.

## Riżultati kurrenti

linja bażi tal-iskema 2 għal 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, paġna 4 KiB,
`data_append_mode=false`, `direct_io=true`. Xogħol fiss:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proġett | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Poġġi throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Ikseb throughput | 5,526 QPS | 7,293 QPS | 1.32x |
| Poġġi bandwidth loġiku | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Ikseb bandwidth loġiku | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| L-ogħla RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Użu tal-ħażna | 269.0 MiB | 101.4 MiB | 0.38x |
| Ħażna / data nominali | 2.20x | 0.83x | 0.38x |

Il-kitba ta '1 miljun entrata (10,000 lott) tikkorrispondi għal
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, jiġifieri, id-dejta hija sinkronizzata darba għal kull lott.
Sadid metriċi addizzjonali: Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, Ikteb Amplifikazzjoni `2.57x`
(`proc_io_write` 329 MB / 128 MB kitba loġika), `4.86` tikteb sejħiet tas-sistema għal kull lott.
Il-proporzjon tal-ħażna huwa kkalkolat ibbażat fuq 1M × 128B = 128 MB data nominali; C++ ma jqanqalx GC/compact din id-darba,
Għalhekk fih aktar paġni tal-istorja. Mhuwiex l-istess numru ta 'kalibru bħal "amplifikazzjoni tal-ispazju tad-dejta tas-sopravivenza".
Il-latency tad-denb P99.9 ta' Rust għadu agħar minn C++ (poġġi 223.3 ms vs 86.0 ms, ġib 355.5 ms vs
48.2 ms), il-linja bażi waħda tvarja ħafna, jekk jogħġbok ara r-rapport intern tal-perf għad-dettalji.

**Nota**: C++'s `write-read` tfisser li l-ħajt tal-kitba u l-ħajt tal-qari jimxu fl-istess ħin, u Rust bħalissa
"Kitba kompluta, iftaħ mill-ġdid, u aqra mill-ġdid", m'hemm l-ebda kompetizzjoni tal-kitba fl-istadju tal-qari. Għalhekk il-multiplikatur ta' `write-read` ma jistax jiġi ttrattat bħala
Qari pur / konklużjoni pura tal-kitba; paragun strett għandu jara `scripts/perf_local.sh --symmetric`
Żewġ pari ta' riżultati huma output: bil-miktub biss u għall-qari biss. In-numri ta' runner kondiviżi singoli jintużaw biss għar-rigressjoni
It-tendenzi, ir-rilaxxi huma ġġudikati bl-użu tal-medjan ta' `--runs`.