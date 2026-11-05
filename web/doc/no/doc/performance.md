# Ytelse

Last ned `perf-latest` av `fkv_bench` fra fkv-depotet og kjør samme arbeidsmengde på en fast maskin.
Og sammenlign gjennomstrømning, latenskvantiler og topp RSS med en C++-grunnlinje.

Parameterkilden stemmer overens med fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++-grunnlinjen kjøres av orakelmiljøet `simple_bench` og sendes deretter ut i henhold til samme skjema og publiseres til
`perf-baseline` utgivelseselement. Sammenligningsresultatene skrives tilbake til lagerdokumentet, og nettstedet gjengir Markdown direkte.
Ingen JSON-datalag opprettholdes.