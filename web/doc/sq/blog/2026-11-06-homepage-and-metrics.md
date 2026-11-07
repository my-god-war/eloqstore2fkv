---
date: 2026-11-06
summary: Krahasimi i performancës së faqes kryesore, zgjerimi i treguesit të vlerësimit, Faqet e GitHub dhe blogjet shumëgjuhëshe në internet
---

# 2026-11-06: Tabela e performancës së faqes kryesore, zgjerimi i treguesit dhe Faqet

Përditësimi i sotëm fokusohet në rishikimet e performancës dhe lançimet e faqeve.

## Krahasimi i performancës së faqes kryesore

- Faqja kryesore është ndryshuar në përparësi të performancës dhe tabela e krahasimit C++/Rust shfaqet drejtpërdrejt në ekranin e parë.
- Shtohen tre grupe të reja treguesish: gjerësia e brezit logjik, vonesa e bishtit P99.9 dhe amplifikimi hapësinor.
- Shtoi detajet e funksionimit të Rust: Skano, skanim i plotë, amplifikim i shkrimit, numri i thirrjeve të sistemit të shkrimit dhe sinkronizimi i të dhënave për grup.
- Shënoni në mënyrë të qartë `write-read` si kalibër të përzier dhe lidhjen me dokumentacionin e performancës.

Baza aktuale e së njëjtës makinë:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Kulmi i RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Përdorimi i hapësirës ruajtëse | 269.0 MiB | 101.4 MiB | 0.38x |

## Treguesit e vlerësimit

Duke iu referuar prodhimit të përbashkët të RocksDB, LevelDB, YCSB, WiredTiger dhe standardeve të tjera, unë shtova:

- Gjerësia e brezit të leximit dhe shkrimit logjik MiB/s;
- P99.9 vonesa e bishtit;
- Amplifikimi i hapësirës, ​​amplifikim shkrimi;
- Numri i thirrjeve të sistemit të shkrimit dhe sinkronizimi i të dhënave në çdo grup;
- Rrjedha e skanimit / skanimit të plotë;
- CPU, gabimet e faqeve, ndërprerësit e kontekstit dhe IO e procesit vazhdojnë të mblidhen nga karriera anësore e burimit.

Doli në pension në të njëjtën kohë`gxhash_4096B_gib_s`. Është një mikrobenchmark hash me një bllok 4 KiB që nuk është në shtegun e motorit KV dhe nuk ka asnjë homolog në C++.

## Korrektësia dhe rregullimet inxhinierike

- Rregulloni hapësirën kryesore të `--rng cpp` read/scan: zbulimi i leximit nuk shumëzohet më me `key_interval` për të shmangur që 75% e leximeve të bien në çelësat e pashkruar.
- Riparimet në pritje të rregulluara të shkaktuara nga vlerësimi i gabuar i macOS `openrsync` i manifesteve me të njëjtën përmbajtje.
- Rregulloi një problem të portës së harduerit ku disqet virtuale QEMU u vlerësuan gabimisht si SSD lokale.

## Faqet dhe Publikimi

- Faqet GitHub të aktivizuara: <https://my-god-war.github.io/eloqstore2fkv/>
- Drejtimi i sajtit mbështet nënshtigje `/eloqstore2fkv/`.
- Në vend të kësaj, dokumenti lexohet nga `raw.githubusercontent.com` dhe 404 i shkaktuar nga mungesa e `web/` në shtegun jsDelivr është fiksuar.
- Navigimi në blog dhe `bun run tran` përkthimi në shumë gjuhë janë të integruara; ky artikull përkthehet automatikisht nga burimi kinez në gjuhë të tjera.

Regjistrimet e mëvonshme të ndryshimeve do të vazhdojnë të dokumentojnë ndryshimet e përputhshmërisë, performancës dhe zinxhirit të mjeteve.