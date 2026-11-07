---
date: 2026-11-06
summary: Porovnání výkonu domovské stránky, rozšíření ukazatelů hodnocení, stránky GitHub a vícejazyčné online blogy
---

# 2026-11-06: Tabulka výkonu domovské stránky, rozšíření indikátoru a stránky

Dnešní aktualizace se zaměřuje na hodnocení výkonu a spouštění webu.

## Porovnání výkonu domovské stránky

- Domovská stránka se změní na prioritu výkonu a na první obrazovce se přímo zobrazí srovnávací tabulka C++/Rust.
- Byly přidány tři nové sady indikátorů: logická šířka pásma, zpoždění P99.9 a prostorové zesílení.
- Přidány podrobnosti o běhu Rustu: Scan, Fullscan, zesílení zápisu, počet systémových volání zápisu a synchronizace dat v dávce.
- Explicitně označte `write-read` jako smíšený kalibr a uveďte odkaz na dokumentaci výkonu.

Aktuální základní linie stejného stroje:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Špičkové RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Využití úložiště | 269.0 MiB | 101.4 MiB | 0.38x |

## Ukazatele hodnocení

S odkazem na společný výstup RocksDB, LevelDB, YCSB, WiredTiger a dalších benchmarků jsem přidal:

- Šířka pásma pro logické čtení a zápis MiB/s;
- P99.9 zpoždění ocasu;
- Zesílení prostoru, zesílení zápisu;
- Počet systémových volání zápisu a synchronizace dat v každé dávce;
- propustnost skenování / úplného skenování;
- CPU, chyby stránek, kontextové přepínače a IO procesu jsou nadále shromažďovány postranním vozíkem prostředků.

Zároveň odešel do důchodu`gxhash_4096B_gib_s`. Jedná se o 4 kiB single block hash microbenchmark, který není na cestě KV motoru a nemá žádný protějšek v C++.

## Korektnost a technické opravy

- Opravte klíčový prostor `--rng cpp` čtení/skenování: detekce čtení již není násobena `key_interval`, aby 75 % čtení nespadlo do nezapsaných klíčů.
- Opraveny pokusy v pohotovostním režimu způsobené chybným úsudkem macOS `openrsync` manifestů se stejným obsahem.
- Opraven problém s hardwarovou bránou, kdy byly virtuální disky QEMU nesprávně posouzeny jako místní SSD.

## Stránky a publikování

- Stránky GitHub povoleny: <https://my-god-war.github.io/eloqstore2fkv/>
- Směrování webu podporuje `/eloqstore2fkv/` dílčí cesty.
- Místo toho se dokument čte z `raw.githubusercontent.com` a chyba 404 způsobená chybějícím `web/` v cestě jsDelivr je opravena.
- Navigace v blogu a `bun run tran` vícejazyčný překlad jsou integrovány; tento článek je automaticky přeložen z čínského zdroje do jiných jazyků.

Následné changelogy budou i nadále dokumentovat kompatibilitu, výkon a změny toolchainu.