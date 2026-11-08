---
date: 2026-11-06
summary: Sammenligning af startsideydelse, udvidelse af evalueringsindikator, GitHub-sider og flersprogede blogs online
---

# 2026-11-06: Hjemmesideydelsestabel, indikatorudvidelse og sider

Dagens opdatering fokuserer på præstationsanmeldelser og webstedslanceringer.

## Hjemmesides ydeevnesammenligning

- Hjemmesiden ændres til præstationsprioritet, og C++/Rust-sammenligningstabellen vises direkte på den første skærm.
- Tre nye sæt indikatorer er tilføjet: logisk båndbredde, P99.9-haleforsinkelse og rumlig forstærkning.
- Tilføjede Rust-kørende detaljer: Scan, Fullscan, skriveforstærkning, antal skrivesystemkald og datasynkronisering pr. batch.
- Markér eksplicit `write-read` som blandet kaliber og link til ydeevnedokumentation.

Nuværende baseline for samme maskine:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Højeste RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Lagerbrug | 269.0 MiB | 101.4 MiB | 0.38x |

## Evalueringsindikatorer

Med henvisning til det almindelige output fra RocksDB, LevelDB, YCSB, WiredTiger og andre benchmarks tilføjede jeg:

- Logisk læse- og skrivebåndbredde MiB/s;
- P99.9 haleforsinkelse;
- Rumforstærkning, skriveforstærkning;
- Antal skrivesystemopkald og datasynkronisering i hver batch;
- Scan / fuld scanning gennemløb;
- CPU, sidefejl, kontekstskift og proces-IO bliver fortsat indsamlet af ressourcesidevognen.

Pensioneret på samme tid`gxhash_4096B_gib_s`. Det er et 4 KiB enkelt blok hash-mikrobenchmark, der ikke er på KV-motorvejen og har ingen C++ modstykke.

## Korrekthed og tekniske rettelser

- Ret nøglerummet for `--rng cpp` læs/scanning: læsedetektion ganges ikke længere med `key_interval` for at undgå, at 75 % af læsningerne falder ind i uskrevne nøgler.
- Rettede standbyforsøg forårsaget af macOS `openrsync` fejlvurdering af manifester med det samme indhold.
- Rettede et hardwaregate-problem, hvor virtuelle QEMU-diske blev fejlbedømt som lokale SSD'er.

## Websteder og udgivelse

- GitHub-sider aktiveret: <https://my-god-war.github.io/eloqstore2fkv/>
- Site routing understøtter `/eloqstore2fkv/` understier.
- Dokumentet læses fra `raw.githubusercontent.com` i stedet, og 404 forårsaget af manglende `web/` i jsDelivr-stien er rettet.
- Blognavigation og `bun run tran` flersproget oversættelse er integreret; denne artikel er automatisk oversat fra den kinesiske kilde til andre sprog.

Efterfølgende changelogs vil fortsat dokumentere kompatibilitet, ydeevne og værktøjskædeændringer.