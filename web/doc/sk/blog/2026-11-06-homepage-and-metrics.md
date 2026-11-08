---
date: 2026-11-06
summary: Porovnanie výkonu domovskej stránky, rozšírenie indikátora hodnotenia, stránky GitHub a viacjazyčné blogy online
---

# 2026-11-06: Tabuľka výkonnosti domovskej stránky, rozšírenie indikátora a stránky

Dnešná aktualizácia sa zameriava na hodnotenie výkonnosti a spúšťanie stránok.

## Porovnanie výkonu domovskej stránky

- Domovská stránka sa zmení na prioritu výkonu a porovnávacia tabuľka C++/Rust sa zobrazí priamo na prvej obrazovke.
- Pridávajú sa tri nové sady indikátorov: logická šírka pásma, oneskorenie P99.9 a priestorové zosilnenie.
- Pridané podrobnosti o behu Rustu: skenovanie, úplné skenovanie, zosilnenie zápisu, počet systémových volaní zápisu a synchronizácia údajov na dávku.
- Explicitne označte `write-read` ako zmiešaný kaliber a uveďte odkaz na dokumentáciu výkonu.

Aktuálna základná línia rovnakého stroja:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Špičkové RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Využitie úložiska | 269.0 MiB | 101.4 MiB | 0.38x |

## Hodnotiace ukazovatele

S odvolaním sa na spoločný výstup RocksDB, LevelDB, YCSB, WiredTiger a ďalších benchmarkov som pridal:

- Šírka pásma logického čítania a zápisu MiB/s;
- P99.9 oneskorenie chvosta;
- Zosilnenie priestoru, zosilnenie zápisu;
- Počet systémových volaní zápisu a synchronizácie údajov v každej dávke;
- Priepustnosť skenovania / úplného skenovania;
- CPU, chyby stránok, prepínače kontextu a IO procesu naďalej zhromažďuje vedľajší nosič prostriedkov.

Zároveň odišiel do dôchodku`gxhash_4096B_gib_s`. Ide o 4 kiB jednoblokový hash mikrobenchmark, ktorý nie je na ceste motora KV a nemá náprotivok v C++.

## Správnosť a technické opravy

- Opravte kľúčový priestor `--rng cpp` čítania/skenovania: detekcia čítania sa už nenásobí `key_interval`, aby sa predišlo tomu, že 75 % čítaní nespadne do nezapísaných kľúčov.
- Opravené opakované pokusy v pohotovostnom režime spôsobené nesprávnym úsudkom macOS `openrsync` manifestov s rovnakým obsahom.
- Opravený problém s hardvérovou bránou, kedy boli virtuálne disky QEMU nesprávne posúdené ako lokálne SSD.

## Stránky a publikovanie

- Stránky GitHub sú povolené: <https://my-god-war.github.io/eloqstore2fkv/>
- Smerovanie lokality podporuje `/eloqstore2fkv/` podcesty.
- Dokument sa namiesto toho číta z `raw.githubusercontent.com` a chyba 404 spôsobená chýbajúcim `web/` v ceste jsDelivr je opravená.
- Navigácia v blogu a `bun run tran` viacjazyčný preklad sú integrované; tento článok je automaticky preložený z čínskeho zdroja do iných jazykov.

Nasledujúce protokoly zmien budú naďalej dokumentovať kompatibilitu, výkon a zmeny v reťazci nástrojov.