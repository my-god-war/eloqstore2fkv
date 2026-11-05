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