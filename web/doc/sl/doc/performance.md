# Zmogljivost

Prenesite `perf-latest` od `fkv_bench` iz repozitorija fkv in zaženite isto delovno obremenitev na fiksnem računalniku.
In primerjajte prepustnost, kvantile zakasnitve in najvišjo vrednost RSS z osnovno linijo C++.

Vir parametra je skladen s fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Osnovno linijo C++ poganja okolje Oracle `simple_bench` in nato izpiše v skladu z isto shemo ter objavi v
`perf-baseline` sprosti sredstvo. Rezultati primerjave se zapišejo nazaj v dokument skladišča in spletno mesto neposredno upodobi Markdown.
Podatkovna plast JSON se ne vzdržuje.