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

## Aktuální výsledky

schéma 2 základní linie pro 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, stránka 4 kB,
`data_append_mode=false`, `direct_io=true`. Pevná pracovní náplň:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Vložte propustnost | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Získejte propustnost | 5,526 QPS | 7,293 QPS | 1.32x |
| Vložte logickou šířku pásma | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Získejte logickou šířku pásma | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Špičkové RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Využití úložiště | 269.0 MiB | 101.4 MiB | 0.38x |
| Úložná / jmenovitá data | 2.20x | 0.83x | 0.38x |

Zápis 1 milionu záznamů (10 000 dávek) odpovídá
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, to znamená, že data jsou synchronizována jednou za dávku.
Další metriky rezu: skenování `141,166 kv/s`, úplné skenování `108,274 kv/s`, zesílení zápisu `2.57x`
(`proc_io_write` 329 MB / 128 MB logických zápisů), `4.86` zápis systémových volání na dávku.
Úložný poměr je vypočítán na základě 1M × 128B = 128 MB nominálních dat; C++ tentokrát nespouští GC/compact,
Obsahuje tedy více stránek historie. Není to stejné číslo kalibru jako "zesílení datového prostoru pro přežití".
Rustova koncová latence P99.9 je stále horší než C++ (uveďte 223,3 ms vs. 86,0 ms, získejte 355,5 ms vs.
48,2 ms), jediná základní linie velmi kolísá, podrobnosti naleznete v interní zprávě o výkonu.

**Poznámka**: C++ `write-read` znamená, že vlákno pro zápis a vlákno pro čtení běží současně a Rust aktuálně
„Dokončete psaní, znovu otevřete a znovu přečtěte“, ve fázi čtení neprobíhá žádná soutěž v psaní. Násobitel `write-read` proto nelze považovat za
Čisté čtení / čisté psaní závěr; přísné srovnání by mělo vidět `scripts/perf_local.sh --symmetric`
Výstupem jsou dvě dvojice výsledků: pouze pro zápis a pouze pro čtení. Jednotlivá sdílená čísla běžců se používají pouze pro regresi
Trendy, verze jsou posuzovány pomocí mediánu `--runs`.