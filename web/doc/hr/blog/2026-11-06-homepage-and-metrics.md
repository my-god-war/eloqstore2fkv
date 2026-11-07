---
date: 2026-11-06
summary: Usporedba performansi početne stranice, proširenje pokazatelja procjene, GitHub stranice i višejezični blogovi na mreži
---

# 2026-11-06: Tablica izvedbe početne stranice, proširenje pokazatelja i stranice

Današnje ažuriranje usredotočeno je na preglede izvedbe i pokretanje web-mjesta.

## Usporedba performansi početne stranice

- Početna stranica mijenja se u prioritet izvedbe, a usporedna tablica C++/Rust izravno se prikazuje na prvom zaslonu.
- Dodana su tri nova skupa indikatora: logička propusnost, P99.9 repno kašnjenje i prostorno pojačanje.
- Dodani detalji o pokretanju Rusta: skeniranje, potpuno skeniranje, pojačanje pisanja, broj sistemskih poziva pisanja i sinkronizacija podataka po seriji.
- Eksplicitno označite `write-read` kao mješoviti kalibar i povežite na dokumentaciju o izvedbi.

Trenutačna osnovna linija za isti stroj:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Korištenje pohrane | 269.0 MiB | 101.4 MiB | 0.38x |

## Pokazatelji evaluacije

Referirajući se na uobičajeni izlaz RocksDB-a, LevelDB-a, YCSB-a, WiredTigera i drugih mjerila, dodao sam:

- Logička propusnost čitanja i pisanja MiB/s;
- P99.9 kašnjenje repa;
- Pojačavanje prostora, pojačanje pisanja;
- Broj sistemskih poziva za pisanje i sinkronizacija podataka u svakoj seriji;
- Protok skeniranja / punog skeniranja;
- CPU, greške na stranici, sklopke konteksta i IO procesa nastavljaju se prikupljati od pomoćne prikolice resursa.

Istovremeno umirovljen`gxhash_4096B_gib_s`. To je 4 KiB hash mikrobenchmark jednog bloka koji nije na putu KV motora i nema C++ pandan.

## Ispravnost i inženjerski popravci

- Popravite prostor ključa za `--rng cpp` čitanje/skeniranje: otkrivanje čitanja više se ne množi s `key_interval` kako bi se izbjeglo da 75% čitanja padne na nenapisane ključeve.
- Ispravljeni ponovni pokušaji u stanju pripravnosti uzrokovani pogrešnom procjenom macOS `openrsync` manifesta s istim sadržajem.
- Riješen je problem hardverskih vrata gdje su QEMU virtualni diskovi pogrešno ocijenjeni kao lokalni SSD-ovi.

## Stranice i izdavaštvo

- GitHub stranice omogućene: <https://my-god-war.github.io/eloqstore2fkv/>
- Usmjeravanje stranice podržava `/eloqstore2fkv/` podputa.
- Dokument se umjesto toga čita iz `raw.githubusercontent.com`, a greška 404 uzrokovana nedostatkom `web/` u putu jsDelivr je popravljena.
- Navigacija blogom i `bun run tran` višejezični prijevod su integrirani; ovaj se članak automatski prevodi s kineskog izvora na druge jezike.

Naknadni zapisnici promjena nastavit će dokumentirati promjene kompatibilnosti, izvedbe i lanca alata.