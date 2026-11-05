# Performanse

Preuzmite `perf-latest` od `fkv_bench` iz fkv repozitorija i pokrenite isto radno opterećenje na fiksnom stroju.
I usporedite propusnost, kvantile latencije i vršni RSS s C++ osnovnom linijom.

Izvor parametra je u skladu s fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ osnovnu liniju pokreće oracle okruženje `simple_bench`, a zatim izlazi prema istoj shemi i objavljuje se
`perf-baseline` otpustite sredstvo. Rezultati usporedbe zapisuju se natrag u skladišni dokument, a stranica izravno renderira Markdown.
Ne održava se JSON podatkovni sloj.