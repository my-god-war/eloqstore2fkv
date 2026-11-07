# Werkverrigting

Laai `perf-latest` van `fkv_bench` van die fkv-bewaarplek af en hardloop dieselfde werklading op 'n vaste masjien.
En vergelyk deurset, latency kwantiles en piek RSS met 'n C++ basislyn.

Die parameter bron stem ooreen met fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Die C++-basislyn word deur die orakel-omgewing `simple_bench` bestuur en word dan volgens dieselfde skema uitgevoer en gepubliseer na
`perf-baseline` stel bate vry. Die vergelykingsresultate word teruggeskryf na die pakhuisdokument, en die webwerf lewer Markdown direk.
Geen JSON-datalaag word onderhou nie.

## Huidige resultate

skema 2-basislyn vir 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB-bladsy,
`data_append_mode=false`, `direct_io=true`. Vaste werklading:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projek | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Sit deurset | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Kry deurset | 5,526 QPS | 7,293 QPS | 1.32x |
| Plaas logiese bandwydte | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Kry logiese bandwydte | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Piek RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Berging gebruik | 269.0 MiB | 101.4 MiB | 0.38x |
| Berging / nominale data | 2.20x | 0.83x | 0.38x |

Die skryf van 1 miljoen inskrywings (10 000 bondels) stem ooreen met
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, dit wil sê, data word een keer per bondel gesinchroniseer.
Roes bykomende maatstawwe: Skandeer `141,166 kv/s`, Fullscan `108,274 kv/s`, Skryfversterking `2.57x`
(`proc_io_write` 329 MB / 128 MB logiese skryfwerk), `4.86` skryf stelseloproepe per bondel.
Die bergingsverhouding word bereken op grond van 1M × 128B = 128 MB nominale data; C++ aktiveer nie hierdie keer GC/compact nie,
Dit bevat dus meer bladsye geskiedenis. Dit is nie dieselfde kalibernommer as "oorlewingsdataruimteversterking" nie.
Rust se P99.9 stert latency is steeds slegter as C++ (stel 223.3 ms vs 86.0 ms, kry 355.5 ms vs.
48.2 ms), wissel die enkele basislyn baie, sien asseblief die interne perf-verslag vir besonderhede.

**Let wel**: C++ se `write-read` beteken dat die skryfdraad en die leesdraad gelyktydig loop, en Rust tans
"Voltooi skryf, heropen, en lees weer", daar is geen skryfkompetisie in die leesstadium nie. Daarom kan die vermenigvuldiger van `write-read` nie as behandel word nie
Suiwer lees / suiwer skryf slot; streng vergelyking moet `scripts/perf_local.sh --symmetric` sien
Twee pare resultate word uitgevoer: slegs skryf en slegs lees. Enkele gedeelde hardlopernommers word slegs vir regressie gebruik
Tendense, vrystellings word beoordeel deur die mediaan van `--runs` te gebruik.