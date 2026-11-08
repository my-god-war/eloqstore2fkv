# Suorituskyky

Lataa `perf-latest` / `fkv_bench` fkv-varastosta ja suorita sama työkuorma kiinteällä koneella.
Ja vertaa suorituskykyä, latenssikvantiileja ja huippu-RSS:ää C++-perustasoon.

Parametrilähde on yhdenmukainen fkv:n kanssa:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Oracle-ympäristö `simple_bench` suorittaa C++-perusviivan ja tulostetaan sitten saman skeeman mukaisesti ja julkaistaan
`perf-baseline` vapauta sisältö. Vertailutulokset kirjoitetaan takaisin varastoasiakirjaan, ja sivusto tekee suoraan Markdownin.
JSON-tietokerrosta ei ylläpidetä.

## Tämänhetkiset tulokset

skeeman 2 perusviiva 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB sivu,
`data_append_mode=false`, `direct_io=true`. Kiinteä työmäärä:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| hanke | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Laita läpimeno | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Hanki läpimeno | 5,526 QPS | 7,293 QPS | 1.32x |
| Aseta looginen kaistanleveys | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Hanki looginen kaistanleveys | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Huippu RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Tallennustilan käyttö | 269.0 MiB | 101.4 MiB | 0.38x |
| Tallennus / nimellistiedot | 2.20x | 0.83x | 0.38x |

1 miljoonan merkinnän kirjoittaminen (10 000 erää) vastaa
`io_write_at_num=48,625`, `io_data_sync_num=10,000` eli tiedot synkronoidaan kerran erää kohden.
Ruosteen lisämittarit: skannaus `141,166 kv/s`, täysskannaus `108,274 kv/s`, kirjoitusvahvistus `2.57x`
(`proc_io_write` 329 Mt / 128 Mt loogista kirjoitusta), `4.86` kirjoita järjestelmäkutsuja erää kohti.
Tallennussuhde on laskettu 1M × 128B = 128 Mt nimellistietojen perusteella; C++ ei laukaise GC/compactia tällä kertaa,
Joten se sisältää enemmän sivuja historiaa. Se ei ole sama kaliiperiluku kuin "selviytymistietotilan vahvistus".
Rustin P99.9:n hännän latenssi on edelleen huonompi kuin C++ (laita 223,3 ms vs 86,0 ms, hanki 355,5 ms vs.
48,2 ms), yksittäinen perusviiva vaihtelee suuresti, katso lisätietoja sisäisestä suorituskykyraportista.

**Huomaa**: C++:n `write-read` tarkoittaa, että kirjoitussäie ja lukusäie toimivat samanaikaisesti ja Rust tällä hetkellä
"Kirjoita loppuun, avaa uudelleen ja lue uudelleen", lukuvaiheessa ei ole kirjoituskilpailua. Siksi `write-read`:n kerrointa ei voida käsitellä
Puhdas lukeminen / puhdas kirjoittaminen; tiukan vertailun pitäisi nähdä `scripts/perf_local.sh --symmetric`
Kaksi tulosparia tulostetaan: vain kirjoitus ja vain luku. Yhden jaetun juoksijan numeroita käytetään vain regressioon
Trendit ja julkaisut arvioidaan käyttämällä mediaania `--runs`.