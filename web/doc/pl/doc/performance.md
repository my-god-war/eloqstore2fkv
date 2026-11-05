# Wydajność

Pobierz `perf-latest` z `fkv_bench` z repozytorium fkv i uruchom to samo zadanie na stałej maszynie.
Porównaj także przepustowość, kwantyle opóźnień i szczytowe wartości RSS z wartością bazową C++.

Źródło parametru jest zgodne z fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Linia bazowa C++ jest uruchamiana przez środowisko Oracle `simple_bench`, a następnie wyprowadzana zgodnie z tym samym schematem i publikowana w
`perf-baseline` zwolnij zasób. Wyniki porównania są zapisywane z powrotem w dokumencie magazynowym, a witryna bezpośrednio renderuje Markdown.
Nie jest utrzymywana żadna warstwa danych JSON.