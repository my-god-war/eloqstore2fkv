---
date: 2026-11-06
summary: Kezdőlap teljesítmény-összehasonlítása, értékelési mutató bővítése, GitHub oldalak és többnyelvű blogok online
---

# 2026-11-06: Főoldal teljesítménytáblázata, mutatóbővítés és oldalak

A mai frissítés a teljesítményértékelésekre és a webhelyindításokra összpontosít.

## A kezdőlap teljesítményének összehasonlítása

- A kezdőlap teljesítményprioritásra vált, és a C++/Rust összehasonlító táblázat közvetlenül megjelenik az első képernyőn.
- Három új jelzőkészlet került hozzáadásra: logikai sávszélesség, P99.9 farokkésleltetés és térbeli erősítés.
- Hozzáadott Rust futtatási részletek: Scan, Fullscan, íráserősítés, írási rendszerhívások száma és adatszinkronizálás kötegenként.
- Explicit módon jelölje meg a `write-read` elemet vegyes kaliberként, és hivatkozzon a teljesítménydokumentációra.

Jelenlegi azonos gép alapállapota:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Csúcs RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Tárhelyhasználat | 269.0 MiB | 101.4 MiB | 0.38x |

## Értékelési mutatók

A RocksDB, LevelDB, YCSB, WiredTiger és más benchmarkok közös kimenetére hivatkozva hozzátettem:

- Logikai olvasási és írási sávszélesség MiB/s;
- P99.9 farokkésleltetés;
- Térerősítés, íráserősítés;
- Az írási rendszerhívások és az adatszinkronizálások száma az egyes kötegekben;
- Scan / Fullscan átviteli sebesség;
- A CPU-t, az oldalhibákat, a kontextuskapcsolókat és a folyamat-IO-t továbbra is az erőforrás-oldalkocsi gyűjti össze.

Ezzel egyidőben nyugdíjba vonult`gxhash_4096B_gib_s`. Ez egy 4 KiB-os egyblokkos hash mikrobenchmark, amely nem szerepel a KV motorpályáján, és nincs C++ megfelelője.

## Korrektség és műszaki javítások

- Javítsa ki az `--rng cpp` olvasási/letapogatási kulcsteret: az olvasásészlelés többé nem lesz szorozva `key_interval`-vel, így elkerülhető, hogy az olvasások 75%-a íratlan kulcsokba kerüljön.
- Javítva a készenléti újrapróbálkozások, amelyeket a macOS `openrsync` tévesen ítéltek meg az azonos tartalmú jegyzékekben.
- Javítva egy hardverkapu probléma, amely miatt a QEMU virtuális lemezeket tévesen helyi SSD-ként értékelték.

## Webhelyek és közzététel

- GitHub-oldalak engedélyezve: <https://my-god-war.github.io/eloqstore2fkv/>
- A webhely útválasztása támogatja az `/eloqstore2fkv/` alútvonalakat.
- A dokumentum ehelyett az `raw.githubusercontent.com`-ből kerül kiolvasásra, és a 404-es hiba, amelyet a jsDelivr útvonalból hiányzó `web/` okoz, kijavítva.
- A blognavigáció és a `bun run tran` többnyelvű fordítás integrálva van; ezt a cikket a rendszer automatikusan lefordítja a kínai forrásból más nyelvekre.

A későbbi változásnaplók továbbra is dokumentálják a kompatibilitást, a teljesítményt és az eszközlánc változásait.