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