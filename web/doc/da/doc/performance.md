# Ydeevne

Download `perf-latest` af `fkv_bench` fra fkv-lageret og kør den samme arbejdsbelastning på en fast maskine.
Og sammenlign gennemløb, latenskvantiler og peak RSS med en C++-baseline.

Parameterkilden stemmer overens med fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++-grundlinjen køres af oracle-miljøet `simple_bench` og udlæses derefter i henhold til det samme skema og udgives til
`perf-baseline` frigivelsesaktiv. Sammenligningsresultaterne skrives tilbage til lagerdokumentet, og webstedet gengiver Markdown direkte.
Intet JSON-datalag opretholdes.

## Aktuelle resultater

skema 2-grundlinje for 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB side,
`data_append_mode=false`, `direct_io=true`. Fast arbejdsbyrde:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Sæt gennemløb | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Få gennemløb | 5,526 QPS | 7,293 QPS | 1.32x |
| Sæt logisk båndbredde | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Få logisk båndbredde | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Højeste RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Lagerbrug | 269.0 MiB | 101.4 MiB | 0.38x |
| Opbevaring / nominelle data | 2.20x | 0.83x | 0.38x |

At skrive 1 million poster (10.000 batches) svarer til
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, dvs. data synkroniseres én gang pr. batch.
Yderligere rustmålinger: Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, skriveforstærkning `2.57x`
(`proc_io_write` 329 MB / 128 MB logisk skrivning), `4.86` skriv systemkald pr. batch.
Lagerforholdet er beregnet ud fra 1M × 128B = 128 MB nominelle data; C++ udløser ikke GC/compact denne gang,
Så den indeholder flere sider med historie. Det er ikke det samme kalibernummer som "survival data space amplification".
Rusts P99.9 hale latency er stadig værre end C++ (sæt 223,3 ms vs 86,0 ms, få 355,5 ms vs.
48,2 ms), den enkelte baseline svinger meget, se venligst den interne perf-rapport for detaljer.

**Bemærk**: C++'s `write-read` betyder, at skrivetråden og læsetråden kører på samme tid, og Rust pt.
"Skriv færdig, genåbn og læs igen", der er ingen skrivekonkurrence i læsestadiet. Derfor kan multiplikatoren af ​​`write-read` ikke behandles som
Ren læsning / ren skrivekonklusion; streng sammenligning bør se `scripts/perf_local.sh --symmetric`
To par resultater udlæses: skrivebeskyttet og skrivebeskyttet. Enkelte delte løbernumre bruges kun til regression
Tendenser, udgivelser bedømmes ved hjælp af medianen af ​​`--runs`.