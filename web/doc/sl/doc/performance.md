# Zmogljivost

Prenesite `perf-latest` od `fkv_bench` iz repozitorija fkv in zaženite isto delovno obremenitev na fiksnem računalniku.
In primerjajte prepustnost, kvantile zakasnitve in najvišjo vrednost RSS z osnovno linijo C++.

Vir parametra je skladen s fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Osnovno linijo C++ poganja okolje Oracle `simple_bench` in nato izpiše v skladu z isto shemo ter objavi v
`perf-baseline` sprosti sredstvo. Rezultati primerjave se zapišejo nazaj v dokument skladišča in spletno mesto neposredno upodobi Markdown.
Podatkovna plast JSON se ne vzdržuje.

## Trenutni rezultati

osnovna shema 2 za 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB stran,
`data_append_mode=false`, `direct_io=true`. Fiksna delovna obremenitev:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Postavite prepustnost | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Pridobite prepustnost | 5,526 QPS | 7,293 QPS | 1.32x |
| Postavite logično pasovno širino | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Pridobite logično pasovno širino | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Vrhunec RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Poraba prostora za shranjevanje | 269.0 MiB | 101.4 MiB | 0.38x |
| Shranjevanje/nominalni podatki | 2.20x | 0.83x | 0.38x |

Pisanje 1 milijona vnosov (10.000 paketov) ustreza
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, kar pomeni, da se podatki sinhronizirajo enkrat na paket.
Dodatne meritve Rust: skeniranje `141,166 kv/s`, polno skeniranje `108,274 kv/s`, povečanje zapisovanja `2.57x`
(`proc_io_write` 329 MB / 128 MB logičnih zapisov), `4.86` pisanje sistemskih klicev na paket.
Razmerje shranjevanja je izračunano na podlagi 1M × 128B = 128 MB nominalnih podatkov; C++ tokrat ne sproži GC/compact,
Torej vsebuje več strani zgodovine. To ni isto število kalibra kot "ojačitev prostora podatkov o preživetju".
Zakasnitev repa Rusta P99.9 je še vedno slabša od C++ (navedite 223,3 ms proti 86,0 ms, dobite 355,5 ms proti
48,2 ms), ena osnovna črta močno niha, za podrobnosti glejte interno poročilo o zmogljivosti.

**Opomba**: `write-read` C++ pomeni, da pisalna nit in bralna nit tečeta istočasno, Rust pa trenutno
"Dokončajte pisanje, ponovno odprite in znova preberite", v stopnji branja ni tekmovanja v pisanju. Zato množitelja `write-read` ni mogoče obravnavati kot
Čisto branje / čisto pisanje zaključek; stroga primerjava bi morala videti `scripts/perf_local.sh --symmetric`
Izpišeta se dva para rezultatov: samo za pisanje in samo za branje. Posamezne skupne številke tekačev se uporabljajo samo za regresijo
Trendi, izdaje so ocenjeni z uporabo mediane `--runs`.