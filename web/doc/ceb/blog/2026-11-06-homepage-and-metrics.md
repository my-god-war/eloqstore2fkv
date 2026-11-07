---
date: 2026-11-06
summary: Ang pagtandi sa pasundayag sa home page, pagpalapad sa indikasyon sa ebalwasyon, Mga Pahina sa GitHub ug mga blog nga daghang pinulongan online
---

# 2026-11-06: Home page performance table, indicator expansion ug Pages

Ang pag-update karon nagpunting sa mga pagsusi sa pasundayag ug paglansad sa site.

## Pagtandi sa performance sa home page

- Ang homepage giusab ngadto sa performance priority, ug ang C++/Rust comparison table direktang gipakita sa unang screen.
- Tulo ka bag-ong set sa indicators ang gidugang: logical bandwidth, P99.9 tail delay, ug spatial amplification.
- Gidugang nga Rust running nga mga detalye: Scan, Fullscan, pagsulat sa pagpadako, gidaghanon sa pagsulat sa mga tawag sa sistema ug pag-synchronize sa data kada batch.
- Tin-aw nga markahan ang `write-read` isip nagkasagol nga kalibre ug sumpay sa dokumentasyon sa pasundayag.

Kasamtangang same-machine baseline:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Kinatas-ang RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Paggamit sa pagtipig | 269.0 MiB | 101.4 MiB | 0.38x |

## Mga timailhan sa ebalwasyon

Naghisgot sa sagad nga output sa RocksDB, LevelDB, YCSB, WiredTiger ug uban pang mga benchmark, akong gidugang:

- Lohikal nga pagbasa ug pagsulat bandwidth MiB/s;
- P99.9 nga paglangan sa ikog;
- Pagpadako sa wanang, pagpadako sa pagsulat;
- Gidaghanon sa pagsulat sa mga tawag sa sistema ug pag-synchronize sa datos sa matag batch;
- Pag-scan / Fullscan throughput;
- Ang CPU, mga sayup sa panid, mga switch sa konteksto, ug proseso sa IO padayon nga gikolekta sa sidecar sa kapanguhaan.

Nagretiro sa samang higayon`gxhash_4096B_gib_s`. Kini usa ka 4 KiB single block hash microbenchmark nga wala sa KV engine path ug walay C++ counterpart.

## Pagkatul-id ug pag-ayo sa engineering

- Ayuhon ang yawe nga luna sa `--rng cpp` read/scan: read detection dili na multiplied sa `key_interval` aron malikayan ang 75% sa mga reads nga mahulog sa dili sinulat nga mga yawe.
- Giayo nga standby rettry tungod sa macOS `openrsync` sayop nga paghukom sa mga manifest nga adunay parehas nga sulud.
- Giayo ang isyu sa hardware gate diin ang mga virtual disk sa QEMU nasayop nga gihukman isip mga lokal nga SSD.

## Mga Site ug Pagmantala

- GitHub Pages enabled: <https://my-god-war.github.io/eloqstore2fkv/>
- Ang pagruta sa site nagsuporta sa `/eloqstore2fkv/` mga subpath.
- Ang dokumento gibasa gikan sa `raw.githubusercontent.com` hinoon, ug ang 404 tungod sa nawala nga `web/` sa jsDelivr nga dalan naayo.
- Ang nabigasyon sa blog ug `bun run tran` multi-language translation gihiusa; kini nga artikulo awtomatik nga gihubad gikan sa Chinese nga tinubdan ngadto sa ubang mga pinulongan.

Ang sunod nga changelogs magpadayon sa pagdokumento sa pagkaangay, pasundayag, ug mga pagbag-o sa toolchain.