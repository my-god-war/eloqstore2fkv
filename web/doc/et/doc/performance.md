# Jõudlus

Laadige fkv hoidlast alla `perf-latest` / `fkv_bench` ja käivitage sama töökoormus fikseeritud masinas.
Ja võrrelge läbilaskevõimet, latentsuskvantiile ja tipp-RSS-i C++ baastasemega.

Parameetri allikas on kooskõlas fkv-ga:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ baasjoont käivitab Oracle'i keskkond `simple_bench` ja seejärel väljastatakse sama skeemi järgi ja avaldatakse
`perf-baseline` vabastage vara. Võrdlustulemused kirjutatakse tagasi laodokumenti ja sait renderdab otse Markdowni.
JSON-i andmekihti ei hooldata.

## Praegused tulemused

skeemi 2 baasjoon 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB leht,
`data_append_mode=false`, `direct_io=true`. Fikseeritud töökoormus:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Pane läbilaskevõime | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Hankige läbilaskevõime | 5,526 QPS | 7,293 QPS | 1.32x |
| Pane loogiline ribalaius | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Hankige loogiline ribalaius | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Tipp RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Salvestusruumi kasutamine | 269.0 MiB | 101.4 MiB | 0.38x |
| Salvestus/nimiandmed | 2.20x | 0.83x | 0.38x |

1 miljoni kirje (10 000 partii) kirjutamine vastab
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, see tähendab, et andmed sünkroonitakse üks kord partii kohta.
Rooste lisamõõdikud: skannimine `141,166 kv/s`, täisskannimine `108,274 kv/s`, kirjutamise võimendus `2.57x`
(`proc_io_write` 329 MB / 128 MB loogilisi kirjutisi), `4.86` kirjutada süsteemikutseid partii kohta.
Salvestussuhe arvutatakse 1M × 128B = 128 MB nimiandmete põhjal; C++ ei käivita seekord GC/compacti,
Nii et see sisaldab rohkem lehekülgi ajalugu. See ei ole sama kaliibrinumber kui "ellujäämisandmete ruumi võimendus".
Rooste P99.9 saba latentsus on endiselt halvem kui C++ (panna 223,3 ms vs 86,0 ms, saada 355,5 ms vs
48,2 ms), üks lähtejoon kõigub suuresti, vaadake üksikasju sisemisest toimivuse aruandest.

**Märkus**: C++ `write-read` tähendab, et kirjutuslõng ja lugemislõng töötavad samal ajal ja Rust hetkel
"Lõpeta kirjutamine, ava uuesti ja loe uuesti", lugemisfaasis kirjutamisvõistlust ei toimu. Seetõttu ei saa `write-read` kordajat käsitleda kui
Puhas lugemine / puhas kirjutamine järeldus; range võrdlus peaks nägema `scripts/perf_local.sh --symmetric`
Väljastatakse kaks paari tulemusi: ainult kirjutatav ja kirjutuskaitstud. Ühte jagatud jooksja numbreid kasutatakse ainult regressiooniks
Trendide ja väljalaske hindamisel kasutatakse `--runs` mediaani.