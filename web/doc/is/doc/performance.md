# Afköst

Sæktu `perf-latest` af `fkv_bench` úr fkv geymslunni og keyrðu sama vinnuálag á fastri vél.
Og berðu saman afköst, leynd magn og hámarks RSS við C++ grunnlínu.

Færibreytugjafinn er í samræmi við fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ grunnlínan er keyrð af véfréttaumhverfinu `simple_bench` og síðan útgefin samkvæmt sama skema og birt á
`perf-baseline` losunareign. Samanburðarniðurstöðurnar eru skrifaðar aftur í vöruhúsaskjalið og vefsvæðið gefur Markdown beint.
Ekkert JSON gagnalag er viðhaldið.

## Núverandi úrslit

grunnlína skema 2 fyrir 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB síða,
`data_append_mode=false`, `direct_io=true`. Fast vinnuálag:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| verkefni | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Settu afköst | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Fáðu gegnumstreymi | 5,526 QPS | 7,293 QPS | 1.32x |
| Settu rökrétt bandbreidd | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Fáðu rökrétta bandbreidd | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Hámarks RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Geymslunotkun | 269.0 MiB | 101.4 MiB | 0.38x |
| Geymsla / nafngögn | 2.20x | 0.83x | 0.38x |

Að skrifa 1 milljón færslur (10.000 lotur) samsvarar
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, það er, gögn eru samstillt einu sinni í hverri lotu.
Ryðviðbótarmælingar: Skanna `141,166 kv/s`, Fullscan `108,274 kv/s`, skrifa mögnun `2.57x`
(`proc_io_write` 329 MB / 128 MB rökrétt skrif), `4.86` skrifa kerfiskall í hverja lotu.
Geymsluhlutfallið er reiknað út frá 1M × 128B = 128 MB nafngögnum; C++ kveikir ekki á GC/compact að þessu sinni,
Svo það inniheldur fleiri síður af sögu. Það er ekki sama kalibernúmer og "survival data space amplification".
P99.9 hala biðtími Rust er enn verri en C++ (settu 223,3 ms á móti 86,0 ms, fáðu 355,5 ms á móti
48,2 ms), ein grunnlínan sveiflast mjög, vinsamlegast sjáðu innri perf skýrsluna fyrir frekari upplýsingar.

**Athugið**: `write-read` C++ þýðir að ritþráðurinn og lesþráðurinn keyra á sama tíma og Rust eins og er
„Kláraðu að skrifa, opnaðu aftur og lestu aftur“, það er engin ritsamkeppni á lestrarstigi. Því er ekki hægt að meðhöndla margfaldara `write-read` sem
Hreinn lestur / hrein skrif niðurstaða; strangur samanburður ætti að sjá `scripts/perf_local.sh --symmetric`
Tvö pör af niðurstöðum eru birt: skrifvarinn og skrifvarinn. Einstök sameiginleg hlauparanúmer eru aðeins notuð fyrir afturför
Stefna, útgáfur eru metnar með miðgildi `--runs`.