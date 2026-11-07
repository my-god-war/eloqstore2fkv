---
date: 2026-11-06
summary: Pagrindinio puslapio našumo palyginimas, vertinimo rodiklio išplėtimas, GitHub puslapiai ir kelių kalbų tinklaraščiai internete
---

# 2026-11-06: Pagrindinio puslapio našumo lentelė, rodiklio išplėtimas ir puslapiai

Šiandienos atnaujinimas skirtas našumo apžvalgoms ir svetainių paleidimams.

## Pagrindinio puslapio našumo palyginimas

- Pagrindinis puslapis pakeičiamas į našumo prioritetą, o C++/Rust palyginimo lentelė tiesiogiai rodoma pirmame ekrane.
- Pridedami trys nauji indikatorių rinkiniai: loginis dažnių juostos plotis, P99.9 uodegos delsa ir erdvinis stiprinimas.
- Pridėta „Rust“ veikimo informacija: nuskaitymas, „Fullscan“, rašymo stiprinimas, rašymo sistemos skambučių skaičius ir duomenų sinchronizavimas vienoje partijoje.
- Aiškiai pažymėkite `write-read` kaip mišraus kalibro ir susiekite su veiklos dokumentacija.

Dabartinė tos pačios mašinos bazinė linija:

| indeksas | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Saugyklos naudojimas | 269.0 MiB | 101.4 MiB | 0.38x |

## Vertinimo rodikliai

Atsižvelgdamas į bendrą RocksDB, LevelDB, YCSB, WiredTiger ir kitų etalonų išvestį, pridėjau:

- Loginis skaitymo ir rašymo pralaidumas MiB/s;
- P99.9 uodegos vėlavimas;
- Erdvės stiprinimas, rašymo stiprinimas;
- Rašymo sistemos skambučių skaičius ir duomenų sinchronizavimas kiekvienoje partijoje;
- Scan / Fullscan pralaidumas;
- CPU, puslapio gedimus, konteksto jungiklius ir proceso IO ir toliau renka resursų šoninė dalis.

Tuo pačiu metu išėjo į pensiją`gxhash_4096B_gib_s`. Tai 4 KiB vieno bloko maišos mikrobenchmarkas, kuris nėra KV variklio kelyje ir neturi C++ atitikmens.

## Teisingumas ir inžineriniai pataisymai

- Pataisykite `--rng cpp` skaitymo / nuskaitymo raktų erdvę: skaitymo aptikimas nebedauginamas iš `key_interval`, kad 75 % skaitymų nepatektų į nerašytus raktus.
- Ištaisyti budėjimo režimo pakartotiniai bandymai, kuriuos sukėlė „macOS“ `openrsync` klaidingas to paties turinio aprašų įvertinimas.
- Ištaisyta aparatinės įrangos vartų problema, kai QEMU virtualieji diskai buvo neteisingai įvertinti kaip vietiniai SSD.

## Svetainės ir leidyba

- „GitHub“ puslapiai įgalinti: <https://my-god-war.github.io/eloqstore2fkv/>
- Svetainės maršrutas palaiko `/eloqstore2fkv/` antrinius kelius.
- Vietoj to dokumentas nuskaitomas iš `raw.githubusercontent.com`, o 404, atsiradęs dėl to, kad jsDelivr kelyje trūksta `web/`, ištaisomas.
- Integruota tinklaraščio naršymas ir `bun run tran` vertimas į kelias kalbas; šis straipsnis automatiškai išverstas iš kinų šaltinio į kitas kalbas.

Vėlesni pakeitimų žurnalai ir toliau dokumentuos suderinamumo, našumo ir įrankių grandinės pakeitimus.