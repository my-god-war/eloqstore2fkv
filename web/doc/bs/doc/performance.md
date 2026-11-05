# Performanse

Preuzmite `perf-latest` od `fkv_bench` iz fkv spremišta i pokrenite isto radno opterećenje na fiksnoj mašini.
I uporedite propusnost, kvantile kašnjenja i vršni RSS sa osnovnom linijom C++.

Izvor parametra je u skladu sa fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ osnovnu liniju pokreće oracle okruženje `simple_bench`, a zatim izlazi prema istoj šemi i objavljuje se na
`perf-baseline` izdanje imovine. Rezultati poređenja se zapisuju nazad u dokument skladišta, a stranica direktno prikazuje Markdown.
Ne održava se JSON sloj podataka.