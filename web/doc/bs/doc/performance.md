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

## Trenutni rezultati

osnovna linija šeme 2 za 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB stranica,
`data_append_mode=false`, `direct_io=true`. Fiksno opterećenje:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekat | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Stavite propusnost | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Dobijte propusnost | 5,526 QPS | 7,293 QPS | 1.32x |
| Stavite logičku propusnost | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Dobijte logičku propusnost | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Upotreba pohrane | 269.0 MiB | 101.4 MiB | 0.38x |
| Skladištenje / nominalni podaci | 2.20x | 0.83x | 0.38x |

Pisanje 1 milion unosa (10.000 serija) odgovara
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, odnosno podaci se sinhronizuju jednom po seriji.
Dodatne metrike za Rust: skeniranje `141,166 kv/s`, puno skeniranje `108,274 kv/s`, pojačanje pisanja `2.57x`
(`proc_io_write` 329 MB / 128 MB logičkog upisivanja), `4.86` pisati sistemske pozive po paketu.
Odnos memorije se izračunava na osnovu 1M × 128B = 128 MB nominalnih podataka; C++ ovaj put ne pokreće GC/compact,
Dakle, sadrži više stranica istorije. To nije isti broj kalibra kao "pojačavanje prostora podataka o preživljavanju".
Rust-ova P99.9 tail latencija je i dalje gora od C++ (stavite 223,3 ms naspram 86,0 ms, dobijete 355,5 ms vs
48,2 ms), pojedinačna osnovna linija uvelike fluktuira, za detalje pogledajte interni izvještaj o perf.

**Napomena**: C++-ov `write-read` znači da se nit pisanja i nit čitanja pokreću u isto vrijeme, a Rust trenutno
"Završi pisanje, ponovo otvori i čitaj ponovo", nema takmičenja u pisanju u fazi čitanja. Stoga se množitelj `write-read` ne može tretirati kao
Čisto čitanje / čisto pisanje zaključak; strogo poređenje bi trebalo vidjeti `scripts/perf_local.sh --symmetric`
Izlaze se dva para rezultata: samo za pisanje i samo za čitanje. Pojedinačni zajednički brojevi trkača koriste se samo za regresiju
Trendovi, izdanja se ocjenjuju pomoću medijane `--runs`.