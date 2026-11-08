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

## Aktualne wyniki

schemat 2 bazowy dla 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, strona 4 KiB,
`data_append_mode=false`, `direct_io=true`. Stałe obciążenie:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Przepustowość | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Uzyskaj przepustowość | 5,526 QPS | 7,293 QPS | 1.32x |
| Umieść przepustowość logiczną | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Uzyskaj przepustowość logiczną | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Szczyt RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Wykorzystanie pamięci | 269.0 MiB | 101.4 MiB | 0.38x |
| Przechowywanie / dane nominalne | 2.20x | 0.83x | 0.38x |

Zapisanie 1 miliona wpisów (10 000 partii) odpowiada
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, czyli dane są synchronizowane raz na partię.
Dodatkowe metryki Rust: skanowanie `141,166 kv/s`, pełne skanowanie `108,274 kv/s`, wzmocnienie zapisu `2.57x`
(`proc_io_write` 329 MB / 128 MB zapisów logicznych), `4.86` zapisu wywołań systemowych na partię.
Współczynnik przechowywania jest obliczany na podstawie 1M × 128B = 128 MB danych nominalnych; C++ tym razem nie uruchamia GC/compact,
Zawiera więc więcej stron historii. To nie jest ten sam kaliber, co „wzmocnienie przestrzeni danych o przetrwaniu”.
Opóźnienie ogona Rusta P99.9 jest wciąż gorsze niż C++ (włóż 223,3 ms w porównaniu z 86,0 ms, uzyskaj 355,5 ms w porównaniu z
48,2 ms), pojedyncza linia bazowa ulega znacznym wahaniom. Aby uzyskać szczegółowe informacje, zapoznaj się z wewnętrznym raportem wydajności.

**Uwaga**: `write-read` w C++ oznacza, że ​​wątek piszący i wątek czytający działają w tym samym czasie, a obecnie Rust
„Dokończ pisanie, otwórz ponownie i przeczytaj ponownie” na etapie czytania nie ma konkursu pisarskiego. Dlatego mnożnik `write-read` nie może być traktowany jako
Czyste czytanie / czyste pisanie wniosków; ścisłe porównanie należy zobaczyć `scripts/perf_local.sh --symmetric`
Wyprowadzane są dwie pary wyników: tylko do zapisu i tylko do odczytu. Pojedyncze wspólne numery biegaczy są używane tylko do regresji
Trendy i publikacje oceniane są na podstawie mediany `--runs`.