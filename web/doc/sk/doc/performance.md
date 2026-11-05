# Výkon

Stiahnite si `perf-latest` z `fkv_bench` z úložiska fkv a spustite rovnakú záťaž na pevnom počítači.
A porovnajte priepustnosť, kvantily latencie a špičkové RSS so základnou líniou C++.

Zdroj parametra je v súlade s fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Základná línia C++ je spustená prostredím oracle `simple_bench` a potom výstup podľa rovnakej schémy a zverejnený na
`perf-baseline` vydať dielo. Výsledky porovnania sa zapíšu späť do skladového dokumentu a stránka priamo vykreslí Markdown.
Nie je udržiavaná žiadna dátová vrstva JSON.