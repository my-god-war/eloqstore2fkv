# Prestanda

Ladda ner `perf-latest` av `fkv_bench` från fkv-förvaret och kör samma arbetsbelastning på en fast maskin.
Och jämför genomströmning, latenskvantiler och topp-RSS med en C++-baslinje.

Parameterkällan överensstämmer med fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++-baslinjen körs av orakelmiljön `simple_bench` och matas sedan ut enligt samma schema och publiceras till
`perf-baseline` releasetillgång. Jämförelseresultaten skrivs tillbaka till lagerdokumentet och webbplatsen återger Markdown direkt.
Inget JSON-datalager bibehålls.

## Aktuella resultat

schema 2 baslinje för 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB-sida,
`data_append_mode=false`, `direct_io=true`. Fast arbetsbelastning:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Sätt genomströmning | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Få genomströmning | 5,526 QPS | 7,293 QPS | 1.32x |
| Sätt logisk bandbredd | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Få logisk bandbredd | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Topp RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Användning av lagring | 269.0 MiB | 101.4 MiB | 0.38x |
| Lagring / nominell data | 2.20x | 0.83x | 0.38x |

Att skriva 1 miljon poster (10 000 partier) motsvarar
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, det vill säga data synkroniseras en gång per batch.
Ytterligare rostmått: Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, Skrivförstärkning `2.57x`
(`proc_io_write` 329 MB / 128 MB logiska skrivningar), `4.86` skriv systemanrop per batch.
Lagringsförhållandet beräknas baserat på 1M × 128B = 128 MB nominell data; C++ utlöser inte GC/compact den här gången,
Så den innehåller fler sidor med historia. Det är inte samma kalibernummer som "survival data space amplification".
Rusts P99.9 svanslatens är fortfarande sämre än C++ (sätta 223,3 ms vs 86,0 ms, få 355,5 ms vs.
48,2 ms), den enda baslinjen fluktuerar kraftigt, se den interna perf-rapporten för detaljer.

**Obs**: C++s `write-read` betyder att skrivtråden och lästråden körs samtidigt, och Rust för närvarande
"Skriv färdigt, öppna igen och läs igen", det finns ingen skrivtävling i lässtadiet. Därför kan multiplikatorn för `write-read` inte behandlas som
Ren läsning / ren skrivavslutning; strikt jämförelse bör se `scripts/perf_local.sh --symmetric`
Två par resultat matas ut: skrivskyddad och skrivskyddad. Enstaka delade löparnummer används endast för regression
Trender, releaser bedöms med medianen `--runs`.