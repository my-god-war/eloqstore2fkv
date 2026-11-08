---
date: 2026-11-06
summary: Poređenje performansi početne stranice, proširenje indikatora evaluacije, GitHub stranice i višejezični blogovi na mreži
---

# 2026-11-06: Tabela učinka početne stranice, proširenje indikatora i stranice

Današnje ažuriranje se fokusira na preglede performansi i lansiranja web lokacija.

## Poređenje performansi početne stranice

- Početna stranica je promijenjena u prioritet performansi, a tablica poređenja C++/Rust je direktno prikazana na prvom ekranu.
- Dodata su tri nova skupa indikatora: logički propusni opseg, P99.9 kašnjenje repa i prostorno pojačanje.
- Dodati detalji o pokretanju Rusta: skeniranje, Fullscan, pojačanje pisanja, broj sistemskih poziva za pisanje i sinhronizacija podataka po seriji.
- Eksplicitno označite `write-read` kao mješoviti kalibar i povežite se sa dokumentacijom o performansama.

Trenutna osnovna linija za istu mašinu:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Upotreba pohrane | 269.0 MiB | 101.4 MiB | 0.38x |

## Indikatori evaluacije

Pozivajući se na uobičajeni izlaz RocksDB, LevelDB, YCSB, WiredTiger i drugih mjerila, dodao sam:

- Propusnost logičkog čitanja i pisanja MiB/s;
- P99.9 kašnjenje repa;
- Svemirsko pojačanje, pojačanje pisanja;
- Broj sistemskih poziva za pisanje i sinhronizacija podataka u svakoj seriji;
- Propusnost skeniranja / punog skeniranja;
- CPU, greške stranice, prebacivanja konteksta i procesni IO nastavljaju se prikupljati od strane pomoćne prikolice resursa.

U isto vrijeme penzionisan`gxhash_4096B_gib_s`. To je mikrobenchmark jednog bloka od 4 KiB koji nije na putu KV motora i nema C++ pandan.

## Ispravnost i inženjerski popravci

- Popravite prostor ključa `--rng cpp` čitanja/skeniranja: detekcija čitanja se više ne množi sa `key_interval` kako bi se izbjeglo da 75% čitanja padne u nenapisane ključeve.
- Ispravljeni ponovni pokušaji u stanju pripravnosti uzrokovani macOS `openrsync` pogrešnom procjenom manifesta s istim sadržajem.
- Popravljen je problem hardverske kapije gdje su QEMU virtuelni diskovi pogrešno ocijenjeni kao lokalni SSD-ovi.

## Stranice i izdavaštvo

- Omogućene GitHub stranice: <https://my-god-war.github.io/eloqstore2fkv/>
- Usmjeravanje web mjesta podržava `/eloqstore2fkv/` podstaze.
- Umjesto toga, dokument se čita sa `raw.githubusercontent.com`, a 404 uzrokovano nedostatkom `web/` u jsDelivr putanji je popravljeno.
- Navigacija blogom i `bun run tran` višejezični prijevod su integrirani; ovaj članak je automatski preveden sa kineskog izvora na druge jezike.

Naknadni zapisnici promjena će nastaviti dokumentirati promjene kompatibilnosti, performansi i lanca alata.