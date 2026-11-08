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

## Aktuálne výsledky

základná čiara schémy 2 pre 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, strana 4 kB,
`data_append_mode=false`, `direct_io=true`. Pevné pracovné zaťaženie:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projektu | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Dajte priepustnosť | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Získajte priepustnosť | 5,526 QPS | 7,293 QPS | 1.32x |
| Vložte logickú šírku pásma | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Získajte logickú šírku pásma | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Špičkové RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Využitie úložiska | 269.0 MiB | 101.4 MiB | 0.38x |
| Úložný / nominálny údaj | 2.20x | 0.83x | 0.38x |

Napísanie 1 milióna záznamov (10 000 dávok) zodpovedá
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, to znamená, že údaje sa synchronizujú raz za dávku.
Ďalšie metriky pre hrdzu: skenovanie `141,166 kv/s`, úplné skenovanie `108,274 kv/s`, zosilnenie zápisu `2.57x`
(`proc_io_write` 329 MB / 128 MB logických zápisov), `4.86` zapisovať systémové volania na dávku.
Úložný pomer je vypočítaný na základe 1M × 128B = 128 MB nominálnych údajov; C++ tentoraz nespustí GC/compact,
Obsahuje teda viac stránok histórie. Nie je to rovnaké číslo kalibru ako „zosilnenie priestoru dát na prežitie“.
Rustova koncová latencia P99.9 je stále horšia ako C++ (uveďte 223,3 ms oproti 86,0 ms, získajte 355,5 ms oproti
48,2 ms), jediná základná línia veľmi kolíše, podrobnosti nájdete v internej správe o výkonnosti.

**Poznámka**: C++ `write-read` znamená, že vlákno na písanie a vlákno na čítanie bežia súčasne a Rust momentálne
„Dokončiť písanie, znova otvoriť a znova prečítať“, v štádiu čítania neexistuje žiadna súťaž v písaní. Preto multiplikátor `write-read` nemožno považovať za
Čisté čítanie / čisté písanie záver; striktné porovnanie by malo vidieť `scripts/perf_local.sh --symmetric`
Výstupom sú dva páry výsledkov: iba na zápis a iba na čítanie. Jednotlivé zdieľané čísla sa používajú iba na regresiu
Trendy, vydania sa posudzujú pomocou mediánu `--runs`.