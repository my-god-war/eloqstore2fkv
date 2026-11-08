---
date: 2026-11-06
summary: Jämförelse av hemsidans prestanda, expansion av utvärderingsindikatorer, GitHub-sidor och flerspråkiga bloggar online
---

# 2026-11-06: Prestandatabell för startsidan, indikatorexpansion och sidor

Dagens uppdatering fokuserar på prestandarecensioner och webbplatslanseringar.

## Hemsidas prestandajämförelse

- Hemsidan ändras till prestandaprioritet och jämförelsetabellen C++/Rust visas direkt på den första skärmen.
- Tre nya uppsättningar indikatorer läggs till: logisk bandbredd, P99.9 tail delay och rumslig förstärkning.
- Lade till Rust-körningsdetaljer: Scan, Fullscan, skrivförstärkning, antal skrivsystemanrop och datasynkronisering per batch.
- Markera uttryckligen `write-read` som blandad kaliber och länka till prestandadokumentation.

Aktuell baslinje för samma maskin:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Topp RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Användning av lagring | 269.0 MiB | 101.4 MiB | 0.38x |

## Utvärderingsindikatorer

Med hänvisning till den vanliga produktionen av RocksDB, LevelDB, YCSB, WiredTiger och andra riktmärken, la jag till:

- Logisk läs- och skrivbandbredd MiB/s;
- P99.9 svansfördröjning;
- Rymdförstärkning, skrivförstärkning;
- Antal skrivsystemanrop och datasynkronisering i varje batch;
- Scan / Fullscan genomströmning;
- CPU, sidfel, kontextväxlar och process-IO fortsätter att samlas in av resurssidecaren.

Pensionerad samtidigt`gxhash_4096B_gib_s`. Det är ett 4 KiB enkelblocks hash-mikrobenchmark som inte finns på KV-motorvägen och har ingen C++-motsvarighet.

## Korrekthet och tekniska korrigeringar

- Fixa nyckelutrymmet för `--rng cpp` läs/skanna: läsdetektering multipliceras inte längre med `key_interval` för att undvika att 75 % av läsningarna hamnar i oskrivna nycklar.
- Fixade vänteförsök orsakade av macOS `openrsync` felbedömning av manifest med samma innehåll.
- Fixade ett problem med hårdvaruporten där virtuella QEMU-diskar missbedömdes som lokala SSD:er.

## Webbplatser och publicering

- GitHub-sidor aktiverade: <https://my-god-war.github.io/eloqstore2fkv/>
- Webbplatsrouting stöder `/eloqstore2fkv/` undersökvägar.
- Dokumentet läses från `raw.githubusercontent.com` istället, och 404 som orsakas av att `web/` saknas i jsDelivr-sökvägen är fixad.
- Bloggnavigering och `bun run tran` översättning på flera språk är integrerade; denna artikel är automatiskt översatt från den kinesiska källan till andra språk.

Efterföljande ändringsloggar kommer att fortsätta att dokumentera kompatibilitet, prestanda och förändringar i verktygskedjan.