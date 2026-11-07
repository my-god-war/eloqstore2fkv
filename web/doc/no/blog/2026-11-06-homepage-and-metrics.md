---
date: 2026-11-06
summary: Sammenligning av ytelse på hjemmesider, utvidelse av evalueringsindikator, GitHub-sider og flerspråklige blogger på nettet
---

# 2026-11-06: Ytelsestabell for hjemmesider, utvidelse av indikatorer og sider

Dagens oppdatering fokuserer på ytelsesvurderinger og nettstedlanseringer.

## Sammenligning av ytelse på hjemmesiden

- Hjemmesiden endres til ytelsesprioritet, og sammenligningstabellen C++/Rust vises direkte på den første skjermen.
- Tre nye sett med indikatorer er lagt til: logisk båndbredde, P99.9 haleforsinkelse og romlig forsterkning.
- Lagt til rustkjøringsdetaljer: Scan, Fullscan, skriveforsterkning, antall skrivesystemanrop og datasynkronisering per batch.
- Merk `write-read` eksplisitt som blandet kaliber og lenke til ytelsesdokumentasjon.

Gjeldende basislinje for samme maskin:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Topp RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Lagringsbruk | 269.0 MiB | 101.4 MiB | 0.38x |

## Evalueringsindikatorer

Med henvisning til den vanlige produksjonen av RocksDB, LevelDB, YCSB, WiredTiger og andre benchmarks, la jeg til:

- Logisk lese- og skrivebåndbredde MiB/s;
- P99.9 haleforsinkelse;
- Romforsterkning, skriveforsterkning;
- Antall skrivesystemanrop og datasynkronisering i hver batch;
- Skanning / Fullskanning gjennomstrømning;
- CPU, sidefeil, kontekstbrytere og prosess-IO fortsetter å bli samlet inn av ressurssidevognen.

Pensjonist på samme tid`gxhash_4096B_gib_s`. Det er en 4 KiB enkeltblokk-hash-mikrobenchmark som ikke er på KV-motorbanen og har ingen C++-motpart.

## Korrekthet og tekniske rettelser

- Fiks nøkkelrommet til `--rng cpp` les/skanning: lesedeteksjon multipliseres ikke lenger med `key_interval` for å unngå at 75 % av lesningene faller inn i uskrevne nøkler.
- Rettet standby-forsøk forårsaket av macOS `openrsync` feilvurdering av manifester med samme innhold.
- Rettet et maskinvareportproblem der virtuelle QEMU-disker ble feilvurdert som lokale SSD-er.

## Nettsteder og publisering

- GitHub-sider aktivert: <https://my-god-war.github.io/eloqstore2fkv/>
- Nettstedsruting støtter `/eloqstore2fkv/` underbaner.
- Dokumentet leses fra `raw.githubusercontent.com` i stedet, og 404 forårsaket av manglende `web/` i jsDelivr-banen er fikset.
- Bloggnavigering og `bun run tran` flerspråklig oversettelse er integrert; denne artikkelen er automatisk oversatt fra den kinesiske kilden til andre språk.

Påfølgende endringslogger vil fortsette å dokumentere kompatibilitet, ytelse og endringer i verktøykjeden.