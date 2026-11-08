---
date: 2026-11-06
summary: Paragun tal-prestazzjoni tal-paġna tad-dar, espansjoni tal-indikatur tal-evalwazzjoni, Paġni GitHub u blogs b'ħafna lingwi onlajn
---

# 2026-11-06: Tabella tal-prestazzjoni tal-paġna ewlenija, espansjoni tal-indikaturi u Paġni

L-aġġornament tal-lum jiffoka fuq ir-reviżjonijiet tal-prestazzjoni u t-tnedija tas-sit.

## Paragun tal-prestazzjoni tal-paġna tad-dar

- Il-homepage tinbidel għal prijorità ta 'prestazzjoni, u t-tabella ta' tqabbil C++/Rust tintwera direttament fuq l-ewwel skrin.
- Tliet settijiet ġodda ta 'indikaturi huma miżjuda: bandwidth loġiku, P99.9 tail delay, u amplifikazzjoni spazjali.
- Miżjud dettalji tat-tħaddim tas-sadid: Scan, Fullscan, amplifikazzjoni tal-kitba, numru ta 'sejħiet tas-sistema tal-kitba u sinkronizzazzjoni tad-dejta għal kull lott.
- Immarka espliċitament `write-read` bħala kalibru mħallat u link għad-dokumentazzjoni tal-prestazzjoni.

Linja bażi attwali tal-istess magna:

| indiċi | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| L-ogħla RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Użu tal-ħażna | 269.0 MiB | 101.4 MiB | 0.38x |

## Indikaturi ta' valutazzjoni

B'referenza għall-output komuni ta' RocksDB, LevelDB, YCSB, WiredTiger u benchmarks oħra, żidt:

- Loġiku qari u tikteb bandwidth MiB/s;
- P99.9 dewmien tad-denb;
- Amplifikazzjoni tal-ispazju, amplifikazzjoni tal-kitba;
- Numru ta 'sejħiet ta' sistema ta 'kitba u sinkronizzazzjoni tad-dejta f'kull lott;
- Skanjar / Skanjar sħiħ throughput;
- Is-CPU, il-ħsarat fil-paġna, il-bidla fil-kuntest, u l-IO tal-proċess ikomplu jinġabru mis-sidecar tar-riżorsi.

Irtirat fl-istess ħin`gxhash_4096B_gib_s`. Huwa mikrobenchmark tal-hash ta' blokka waħda ta' 4 KiB li mhuwiex fit-triq tal-magna KV u m'għandu l-ebda kontroparti C++.

## Il-korrettezza u l-inġinerija

- Waħħal l-ispazju taċ-ċavetta ta' `--rng cpp` qari/skan: l-iskoperta tal-qari m'għadhiex immultiplikata b'`key_interval` biex tevita li 75% tal-qari jaqgħu f'ċwievet mhux miktuba.
- Provazzjonijiet fissi ta' standby ikkawżati minn macOS `openrsync` ġudizzju ħażin ta' manifesti bl-istess kontenut.
- Irranġa kwistjoni ta' gate hardware fejn id-diski virtwali QEMU ġew iġġudikati ħażin bħala SSDs lokali.

## Siti u Pubblikazzjoni

- Paġni GitHub attivati: <https://my-god-war.github.io/eloqstore2fkv/>
- Ir-rotot tas-sit jappoġġja `/eloqstore2fkv/` submogħdijiet.
- Id-dokument jinqara minn `raw.githubusercontent.com` minflok, u l-404 ikkawżat minn `web/` nieqsa fil-mogħdija jsDelivr hija ffissata.
- In-navigazzjoni tal-blog u `bun run tran` traduzzjoni b'diversi lingwi huma integrati; dan l-artikolu huwa tradott awtomatikament mis-sors Ċiniż għal lingwi oħra.

Changelogs sussegwenti se jkomplu jiddokumentaw il-kompatibbiltà, il-prestazzjoni, u l-bidliet fil-katina tal-għodda.