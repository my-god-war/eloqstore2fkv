---
date: 2026-11-06
summary: Kodulehe toimivuse võrdlus, hindamisnäitajate laiendamine, GitHubi lehed ja mitmekeelsed ajaveebid võrgus
---

# 2026-11-06: Avalehe toimivuse tabel, indikaatorite laiendus ja lehed

Tänane värskendus keskendub toimivuse ülevaatustele ja saidi käivitamisele.

## Avalehe toimivuse võrdlus

- Avaleht muudetakse jõudluse prioriteediks ja C++/Rust võrdlustabel kuvatakse otse esimesel ekraanil.
- Lisatud on kolm uut indikaatorite komplekti: loogiline ribalaius, P99.9 saba viivitus ja ruumiline võimendus.
- Lisatud Rooste käitamise üksikasjad: skannimine, Fullscan, kirjutamise võimendus, kirjutamissüsteemi kõnede arv ja andmete sünkroonimine partii kohta.
- Märkige selgesõnaliselt `write-read` segakaliibriks ja link jõudlusdokumentatsioonile.

Praegune sama masina baasjoon:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Tipp RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Salvestusruumi kasutamine | 269.0 MiB | 101.4 MiB | 0.38x |

## Hindamisnäitajad

Viidates RocksDB, LevelDB, YCSB, WiredTigeri ja muude võrdlusaluste ühisele väljundile, lisasin:

- Loogilise lugemise ja kirjutamise ribalaius MiB/s;
- P99.9 saba viivitus;
- Ruumi võimendamine, kirjutamise võimendus;
- Kirjutamissüsteemi kõnede arv ja andmete sünkroonimine igas partiis;
- Scan / Fullscan läbilaskevõime;
- Ressursi külgkorv kogub jätkuvalt protsessori, lehe tõrkeid, kontekstilüliteid ja protsessi IO-d.

Samal ajal pensionil`gxhash_4096B_gib_s`. See on 4 KiB ühe ploki räsi mikrobenchmark, mis ei asu KV mootoriteel ja millel puudub C++ vaste.

## Korrektsus ja tehnilised parandused

- Parandage `--rng cpp` lugemise/skannimise klahviruum: lugemistuvastust ei korruta enam `key_interval`-ga, et vältida 75% lugemiste sattumist kirjutamata klahvidesse.
- Parandatud ooterežiimi korduskatsed, mille põhjustas macOS `openrsync` sama sisuga manifestide väär hindamine.
- Parandatud riistvaravärava probleem, mille tõttu QEMU virtuaalseid kettaid hinnati valesti kohalikeks SSD-deks.

## Saidid ja avaldamine

- GitHubi lehed on lubatud: <https://my-god-war.github.io/eloqstore2fkv/>
- Saidi marsruutimine toetab `/eloqstore2fkv/` alamteid.
- Selle asemel loetakse dokumenti aadressilt `raw.githubusercontent.com` ja 404, mille põhjustas jsDelivr tee puudu `web/`, parandatakse.
- Blogi navigeerimine ja `bun run tran` mitmekeelne tõlge on integreeritud; see artikkel tõlgitakse automaatselt hiina allikast teistesse keeltesse.

Järgmised muudatuste logid dokumenteerivad jätkuvalt ühilduvuse, jõudluse ja tööriistaahela muudatusi.