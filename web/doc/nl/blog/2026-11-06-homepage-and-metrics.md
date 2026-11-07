---
date: 2026-11-06
summary: Prestatievergelijking van startpagina's, uitbreiding van evaluatie-indicatoren, GitHub-pagina's en meertalige blogs online
---

# 06-11-2026: Prestatietabel startpagina, indicatoruitbreiding en pagina's

De update van vandaag richt zich op prestatiebeoordelingen en site-lanceringen.

## Prestatievergelijking op de startpagina

- De startpagina is gewijzigd in prestatieprioriteit en de C++/Rust-vergelijkingstabel wordt direct op het eerste scherm weergegeven.
- Er zijn drie nieuwe sets indicatoren toegevoegd: logische bandbreedte, P99.9 staartvertraging en ruimtelijke versterking.
- Rust-loopdetails toegevoegd: Scan, Fullscan, schrijfversterking, aantal schrijfsysteemaanroepen en gegevenssynchronisatie per batch.
- Markeer `write-read` expliciet als gemengd kaliber en link naar prestatiedocumentatie.

Huidige basislijn voor dezelfde machine:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Piek RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Opslaggebruik | 269.0 MiB | 101.4 MiB | 0.38x |

## Evaluatie-indicatoren

Verwijzend naar de gemeenschappelijke output van RocksDB, LevelDB, YCSB, WiredTiger en andere benchmarks, heb ik toegevoegd:

- Logische lees- en schrijfbandbreedte MiB/s;
- P99.9 staartvertraging;
- Ruimteversterking, schrijfversterking;
- Aantal schrijfsysteemaanroepen en gegevenssynchronisatie in elke batch;
- Scan-/volledige scandoorvoer;
- CPU, paginafouten, contextwisselingen en proces-IO worden nog steeds verzameld door de bronzijspan.

Tegelijkertijd met pensioen`gxhash_4096B_gib_s`. Het is een hash-microbenchmark van 4 KiB met één blok die zich niet op het KV-engine-pad bevindt en geen C++-tegenhanger heeft.

## Correctheid en technische oplossingen

- Repareer de sleutelruimte van `--rng cpp` lezen/scannen: leesdetectie wordt niet langer vermenigvuldigd met `key_interval` om te voorkomen dat 75% van de leesbewerkingen in ongeschreven sleutels terechtkomt.
- Probleem opgelost met nieuwe pogingen in stand-by die werden veroorzaakt door macOS `openrsync` verkeerde inschatting van manifesten met dezelfde inhoud.
- Er is een probleem met de hardwarepoort opgelost waarbij virtuele QEMU-schijven verkeerd werden beoordeeld als lokale SSD's.

## Sites en publiceren

- GitHub-pagina's ingeschakeld: <https://my-god-war.github.io/eloqstore2fkv/>
- Siteroutering ondersteunt `/eloqstore2fkv/` subpaden.
- Het document wordt in plaats daarvan gelezen vanaf `raw.githubusercontent.com` en de 404 veroorzaakt door het ontbreken van `web/` in het jsDelivr-pad is opgelost.
- Blognavigatie en `bun run tran` meertalige vertaling zijn geïntegreerd; dit artikel wordt automatisch vanuit de Chinese bron naar andere talen vertaald.

Daaropvolgende changelogs zullen de compatibiliteit, prestaties en wijzigingen in de toolchain blijven documenteren.