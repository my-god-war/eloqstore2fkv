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