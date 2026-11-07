# Performanse

Preuzmite `perf-latest` od `fkv_bench` iz fkv repozitorija i pokrenite isto radno opterećenje na fiksnom stroju.
I usporedite propusnost, kvantile latencije i vršni RSS s C++ osnovnom linijom.

Izvor parametra je u skladu s fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ osnovnu liniju pokreće oracle okruženje `simple_bench`, a zatim izlazi prema istoj shemi i objavljuje se
`perf-baseline` otpustite sredstvo. Rezultati usporedbe zapisuju se natrag u skladišni dokument, a stranica izravno renderira Markdown.
Ne održava se JSON podatkovni sloj.

## Trenutni rezultati

osnovna linija sheme 2 za 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, stranica od 4 KiB,
`data_append_mode=false`, `direct_io=true`. Fiksno radno opterećenje:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Stavi propusnost | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Ostvarite propusnost | 5,526 QPS | 7,293 QPS | 1.32x |
| Stavite logičku propusnost | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Dobijte logičnu propusnost | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Korištenje pohrane | 269.0 MiB | 101.4 MiB | 0.38x |
| Pohrana / nazivni podaci | 2.20x | 0.83x | 0.38x |

Pisanje 1 milijun unosa (10 000 serija) odgovara
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, odnosno podaci se sinkroniziraju jednom po paketu.
Dodatne metrike Rusta: skeniranje `141,166 kv/s`, potpuno skeniranje `108,274 kv/s`, pojačanje pisanja `2.57x`
(`proc_io_write` 329 MB / 128 MB logičko pisanje), `4.86` pisanje sistemskih poziva po paketu.
Omjer pohrane izračunat je na temelju 1M × 128B = 128 MB nominalnih podataka; C++ ovaj put ne pokreće GC/compact,
Dakle, sadrži više stranica povijesti. To nije isti broj kalibra kao "povećanje prostora podataka o preživljavanju".
Rustovo P99.9 repno kašnjenje još uvijek je gore od C++ (stavite 223,3 ms naspram 86,0 ms, dobijete 355,5 ms naspram
48,2 ms), pojedinačna osnovna linija uvelike varira, pogledajte interno izvješće o performansama za detalje.

**Napomena**: `write-read` za C++ znači da se nit za pisanje i nit za čitanje izvode u isto vrijeme, a Rust trenutno
"Dovršite pisanje, ponovno otvorite i ponovno pročitajte", u fazi čitanja nema natjecanja u pisanju. Stoga se množitelj `write-read` ne može tretirati kao
Čisto čitanje / čisto pisanje zaključak; stroga usporedba trebala bi vidjeti `scripts/perf_local.sh --symmetric`
Izlaze dva para rezultata: samo za pisanje i samo za čitanje. Pojedinačni zajednički brojevi trkača koriste se samo za regresiju
Trendovi, izdanja ocjenjuju se pomoću medijana `--runs`.