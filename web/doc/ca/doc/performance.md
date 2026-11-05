# Rendiment

Baixeu `perf-latest` de `fkv_bench` des del repositori fkv i executeu la mateixa càrrega de treball en una màquina fixa.
I compareu el rendiment, els quantils de latència i l'RSS màxim amb una línia de base C++.

La font del paràmetre és coherent amb fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

L'entorn d'oracle `simple_bench` executa la línia base de C++ i després es publica segons el mateix esquema i es publica a
`perf-baseline` allibera el recurs. Els resultats de la comparació es tornen a escriure al document de magatzem i el lloc representa directament Markdown.
No es manté cap capa de dades JSON.