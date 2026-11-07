---
date: 2026-11-06
summary: Paghahambing ng pagganap ng home page, pagpapalawak ng tagapagpahiwatig ng pagsusuri, Mga Pahina ng GitHub at mga blog na may maraming wika online
---

# 2026-11-06: talahanayan ng pagganap ng home page, pagpapalawak ng indicator at Mga Pahina

Nakatuon ang update ngayong araw sa mga pagsusuri sa pagganap at paglulunsad ng site.

## Paghahambing ng pagganap ng home page

- Ang homepage ay binago sa performance priority, at ang C++/Rust comparison table ay direktang ipinapakita sa unang screen.
- Tatlong bagong set ng mga indicator ang idinagdag: logical bandwidth, P99.9 tail delay, at spatial amplification.
- Nagdagdag ng Rust running details: Scan, Fullscan, write amplification, number of write system calls at data synchronization bawat batch.
- Tahasang markahan ang `write-read` bilang magkahalong kalibre at i-link sa dokumentasyon ng pagganap.

Kasalukuyang parehong-machine baseline:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Paggamit ng imbakan | 269.0 MiB | 101.4 MiB | 0.38x |

## Mga tagapagpahiwatig ng pagsusuri

Ang pagtukoy sa karaniwang output ng RocksDB, LevelDB, YCSB, WiredTiger at iba pang mga benchmark, idinagdag ko:

- Logical read and write bandwidth MiB/s;
- P99.9 pagkaantala ng buntot;
- Space amplification, pagsulat amplification;
- Bilang ng mga write system call at data synchronization sa bawat batch;
- I-scan / Fullscan throughput;
- Patuloy na kinokolekta ng resource sidecar ang CPU, mga page fault, context switch, at process IO.

Kasabay na nagretiro`gxhash_4096B_gib_s`. Ito ay isang 4 KiB single block hash microbenchmark na wala sa KV engine path at walang C++ na katapat.

## Katumpakan at pag-aayos ng engineering

- Ayusin ang key space ng `--rng cpp` read/scan: read detection ay hindi na i-multiply sa `key_interval` upang maiwasan ang 75% ng mga reads na mahulog sa mga hindi nakasulat na key.
- Inayos ang standby rettry na dulot ng macOS `openrsync` maling paghuhusga sa mga manifest na may parehong nilalaman.
- Inayos ang isyu ng hardware gate kung saan ang mga virtual disk ng QEMU ay napagkamalan bilang mga lokal na SSD.

## Mga Site at Pag-publish

- Pinagana ang GitHub Pages: <https://my-god-war.github.io/eloqstore2fkv/>
- Sinusuportahan ng pagruruta ng site ang `/eloqstore2fkv/` mga subpath.
- Ang dokumento ay binabasa mula sa `raw.githubusercontent.com` sa halip, at ang 404 na dulot ng nawawalang `web/` sa jsDelivr path ay naayos na.
- Ang blog navigation at `bun run tran` multi-language translation ay isinama; ang artikulong ito ay awtomatikong isinalin mula sa Chinese source sa ibang mga wika.

Ang mga kasunod na changelog ay patuloy na magdodokumento ng compatibility, performance, at mga pagbabago sa toolchain.