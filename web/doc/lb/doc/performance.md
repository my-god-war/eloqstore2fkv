# Leeschtung

Luet `perf-latest` vun `fkv_bench` aus dem fkv-Repository erof a lafen déiselwecht Aarbechtslaascht op enger fixer Maschinn.
A vergläicht Duerchgang, Latenzquantilen a Peak RSS mat enger C ++ Baseline.

De Parameter Quell ass konsequent mat fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

D'C++ Baseline gëtt vum Oracle Ëmfeld `simple_bench` geleet an dann no deemselwechte Schema erausginn a publizéiert op
`perf-baseline` Verëffentlechungsaktiv. D'Vergläichsresultater ginn zréck an de Lagerdokument geschriwwe, an de Site gëtt direkt Markdown.
Keng JSON Dateschicht gëtt erhale gelooss.

## Aktuell Resultater

Schema 2 Baseline fir 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB Säit,
`data_append_mode=false`, `direct_io=true`. Fixéiert Aarbechtsbelaaschtung:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| Projet | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Gitt Duerchgang | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Kréien Duerchféierung | 5,526 QPS | 7,293 QPS | 1.32x |
| Setzt logesch Bandbreedung | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Kréien logesch bandwidth | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Héichpunkt RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Stockage Benotzung | 269.0 MiB | 101.4 MiB | 0.38x |
| Stockage / Nominell Daten | 2.20x | 0.83x | 0.38x |

Schreiwen 1 Millioun Entréen (10.000 Chargen) entsprécht
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, dat heescht, Daten ginn eemol pro Batch synchroniséiert.
Rust zousätzlech Metriken: Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, Schreifverstäerkung `2.57x`
(`proc_io_write` 329 MB / 128 MB logesch Schreiwen), `4.86` schreiwen System Uriff pro Batch.
De Späicherverhältnis gëtt berechent baséiert op 1M × 128B = 128 MB nominal Daten; C++ léist dës Kéier net GC/compact aus,
Also enthält et méi Säite vun der Geschicht. Et ass net déiselwecht Kalibernummer wéi "Iwwerliewensdatenraumverstärkung".
Dem Rust seng P99.9 Schwanzlatenz ass nach ëmmer méi schlëmm wéi C++ (setzen 223.3 ms vs 86.0 ms, kritt 355.5 ms vs.
48,2 ms), déi eenzeg Baseline schwankt immens, kuckt w.e.g. den internen Perf Bericht fir Detailer.

**Notiz**: C++'s `write-read` bedeit datt de Schreiwe thread an de Liesthread gläichzäiteg lafen, a Rust aktuell
"Schreiwen komplett, nei op a liest erëm", et gëtt kee Schreifkonkurrenz an der Liesstadium. Dofir kann de Multiplikator vun `write-read` net als behandelt ginn
Pure Liesen / Pure Schreiwen Ofschloss; strenge Verglach soll `scripts/perf_local.sh --symmetric` gesinn
Zwee Pair vu Resultater ginn erausginn: nëmmen schreiwen an nëmmen liesen. Eenzel gedeelt Leefer Zuelen ginn nëmme fir Regressioun benotzt
Trends, Verëffentlechunge gi mat der Median vun `--runs` beurteelt.