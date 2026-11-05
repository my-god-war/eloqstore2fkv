# Výkon

Stáhněte si `perf-latest` z `fkv_bench` z úložiště fkv a spusťte stejnou zátěž na pevném počítači.
A porovnejte propustnost, kvantily latence a špičkové RSS se základní linií C++.

Zdroj parametru je konzistentní s fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Základní linie C++ je spuštěna prostředím oracle `simple_bench` a poté je výstupem podle stejného schématu a publikována do
`perf-baseline` vydání díla. Výsledky porovnání jsou zapsány zpět do skladového dokumentu a web přímo vykresluje Markdown.
Není udržována žádná datová vrstva JSON.