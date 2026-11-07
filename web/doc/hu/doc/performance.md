# Teljesítmény

Töltse le a(z) `perf-latest` / `fkv_bench` `fkv_bench` elemet az fkv tárolóból, és futtassa ugyanazt a terhelést egy rögzített gépen.
És hasonlítsa össze az átviteli sebességet, a késleltetési kvantisokat és a csúcs RSS-t a C++ alapvonallal.

A paraméter forrása összhangban van az fkv-vel:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

A C++ alapvonalat az oracle környezet `simple_bench` futtatja, majd ugyanazon séma szerint adja ki, és közzéteszi
`perf-baseline` kiadási eszköz. Az összehasonlítási eredmények visszaírásra kerülnek a raktári dokumentumba, és a webhely közvetlenül megjeleníti a Markdown-t.
Nincs karbantartott JSON-adatréteg.

## Aktuális eredmények

2. séma alapvonal 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB oldal,
`data_append_mode=false`, `direct_io=true`. Fix munkaterhelés:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projektet | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Tedd áteresztőképességet | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Szerezzen áteresztőképességet | 5,526 QPS | 7,293 QPS | 1.32x |
| Helyezzen logikai sávszélességet | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Szerezzen logikai sávszélességet | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Csúcs RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Tárhelyhasználat | 269.0 MiB | 101.4 MiB | 0.38x |
| Tárolás / névleges adatok | 2.20x | 0.83x | 0.38x |

1 millió bejegyzés (10 000 tétel) írása megfelel
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, azaz az adatok kötegenként egyszer szinkronizálódnak.
További mérőszámok rozsdásodása: Szkennelés `141,166 kv/s`, Teljes szkennelés `108,274 kv/s`, Íráserősítés `2.57x`
(`proc_io_write` 329 MB / 128 MB logikai írások), `4.86` rendszerhívások kötegenkénti írása.
A tárolási arány számítása 1M × 128B = 128 MB névleges adat alapján történik; A C++ ezúttal nem váltja ki a GC/compact-ot,
Tehát több oldalnyi történelem van benne. Nem ugyanaz a kaliberszám, mint a "túlélési adattér-erősítés".
A Rust P99.9 faroklatenciája még mindig rosszabb, mint a C++ (223,3 ms vs 86,0 ms, 355,5 ms vs.
48,2 ms), az egyetlen alapvonal nagymértékben ingadozik, a részletekért lásd a belső teljesítményjelentést.

**Megjegyzés**: A C++ `write-read` azt jelenti, hogy az írószál és az olvasószál egyszerre fut, és a Rust jelenleg
„Fejezze be az írást, nyissa ki újra, és olvassa el újra”, nincs írási verseny az olvasási szakaszban. Ezért az `write-read` szorzóját nem lehet így kezelni
Tiszta olvasás / tiszta írás következtetés; szigorú összehasonlítás esetén a következőt kell látnia: `scripts/perf_local.sh --symmetric`
Két eredménypár jelenik meg: csak írható és csak olvasható. Az egyetlen megosztott futószámok csak regresszióhoz használhatók
A trendeket és a kiadásokat a(z) `--runs` medián alapján ítéljük meg.