---
date: 2026-11-06
summary: Tuisblad prestasie vergelyking, evaluering aanwyser uitbreiding, GitHub Pages en multi-taal blogs aanlyn
---

# 2026-11-06: Tuisbladprestasietabel, aanwyseruitbreiding en bladsye

Vandag se opdatering fokus op prestasiebeoordelings en werfbekendstellings.

## Tuisblad prestasie vergelyking

- Die tuisblad word verander na prestasieprioriteit, en die C++/Rust-vergelykingstabel word direk op die eerste skerm vertoon.
- Drie nuwe stelle aanwysers word bygevoeg: logiese bandwydte, P99.9-stertvertraging en ruimtelike versterking.
- Bygevoeg Roes lopende besonderhede: skandering, Fullscan, skryf versterking, aantal skryf stelsel oproepe en data sinchronisasie per bondel.
- Merk `write-read` uitdruklik as gemengde kaliber en skakel na prestasiedokumentasie.

Huidige dieselfde masjien basislyn:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Piek RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Berging gebruik | 269.0 MiB | 101.4 MiB | 0.38x |

## Evalueringsaanwysers

Met verwysing na die algemene uitset van RocksDB, LevelDB, YCSB, WiredTiger en ander maatstawwe, het ek bygevoeg:

- Logiese lees- en skryfbandwydte MiB/s;
- P99.9 stert vertraging;
- Ruimteversterking, skryfversterking;
- Aantal skryfstelseloproepe en datasinchronisasie in elke bondel;
- Skandeer / Fullscan deurset;
- SVE, bladsyfoute, konteksskakelaars en proses-IO word steeds deur die hulpbronsyspan ingesamel.

Terselfdertyd afgetree`gxhash_4096B_gib_s`. Dit is 'n 4 KiB enkelblok-hash-mikromaatstaf wat nie op die KV-enjinpad is nie en geen C++-eweknie het nie.

## Korrektheid en ingenieursoplossings

- Maak die sleutelspasie van `--rng cpp` lees/skandeer reg: leesbespeuring word nie meer vermenigvuldig met `key_interval` om te verhoed dat 75% van leesstukke in ongeskrewe sleutels val.
- Vaste bystandweerpogings wat veroorsaak word deur macOS `openrsync` verkeerde beoordeling van manifeste met dieselfde inhoud.
- Het 'n hardewarehekprobleem opgelos waar QEMU virtuele skywe as plaaslike SSD's verkeerd beoordeel is.

## Webwerwe en publisering

- GitHub-bladsye geaktiveer: <https://my-god-war.github.io/eloqstore2fkv/>
- Werfroetering ondersteun `/eloqstore2fkv/` subpaaie.
- Die dokument word in plaas daarvan gelees vanaf `raw.githubusercontent.com`, en die 404 wat veroorsaak word deur ontbrekende `web/` in die jsDelivr-pad is reggestel.
- Blognavigasie en `bun run tran` multi-taal vertaling is geïntegreer; hierdie artikel word outomaties uit die Chinese bron in ander tale vertaal.

Daaropvolgende veranderingslogboeke sal voortgaan om versoenbaarheid, werkverrigting en nutskettingveranderinge te dokumenteer.