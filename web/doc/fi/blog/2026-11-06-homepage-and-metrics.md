---
date: 2026-11-06
summary: Kotisivujen suorituskyvyn vertailu, arviointiindikaattorin laajentaminen, GitHub-sivut ja monikieliset blogit verkossa
---

# 2026-11-06: Kotisivun tehokkuustaulukko, indikaattorin laajennus ja sivut

Tämän päivän päivitys keskittyy suorituskykyarviointiin ja sivuston julkaisuihin.

## Kotisivun tehokkuuden vertailu

- Kotisivu vaihtuu suorituskykyprioriteetiksi, ja C++/Rust-vertailutaulukko näkyy suoraan ensimmäisessä näytössä.
- Lisätään kolme uutta indikaattorisarjaa: looginen kaistanleveys, P99.9 tail -viive ja spatiaalinen vahvistus.
- Lisätty Rust-ajotiedot: Scan, Fullscan, kirjoitusvahvistus, kirjoitusjärjestelmäkutsujen määrä ja tietojen synkronointi erää kohti.
- Merkitse `write-read` eri kaliiperiksi ja linkitä suorituskykydokumentaatioihin.

Nykyinen saman koneen perusviiva:

| indeksi | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Huippu RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Tallennustilan käyttö | 269.0 MiB | 101.4 MiB | 0.38x |

## Arviointiindikaattorit

Viitaten RocksDB:n, LevelDB:n, YCSB:n, WiredTigerin ja muiden vertailuarvojen yhteiseen tuottoon, lisäsin:

- Looginen luku- ja kirjoituskaistanleveys MiB/s;
- P99.9 hännän viive;
- Space vahvistus, kirjoittaminen vahvistus;
- Kirjoitusjärjestelmäkutsujen ja datan synkronoinnin määrä kussakin erässä;
- Skannaus / Fullscan suorituskyky;
- Resurssin sivuvaunu kerää edelleen CPU-, sivuviat, kontekstikytkimet ja prosessin IO.

Eläkkeellä samaan aikaan`gxhash_4096B_gib_s`. Se on 4 KiB:n yhden lohkon hash-mikrobenchmark, joka ei ole KV-moottoriradalla eikä sillä ole C++-vastinetta.

## Oikeus ja tekniset korjaukset

- Korjaa `--rng cpp` luku/skannaus -avaintila: lukutunnistusta ei enää kerrota `key_interval`:lla, jotta 75 % luennoista ei joutuisi kirjoittamattomiin avaimiin.
- Korjattu valmiustilan uudelleenyritykset, jotka johtuvat macOS:n `openrsync` virhearvioinnista samansisältöisten luetteloiden kanssa.
- Korjattu laitteistoporttiongelma, jossa QEMU-virtuaalilevyt arvioitiin väärin paikallisiksi SSD-levyiksi.

## Sivustot ja julkaisut

- GitHub-sivut käytössä: <https://my-god-war.github.io/eloqstore2fkv/>
- Sivuston reititys tukee `/eloqstore2fkv/` alipolkuja.
- Asiakirja luetaan sen sijaan osoitteesta `raw.githubusercontent.com`, ja jsDelivr-polun puuttuvasta `web/`:sta johtuva 404 korjataan.
- Blogissa navigointi ja `bun run tran` monikielinen käännös on integroitu; tämä artikkeli käännetään automaattisesti kiinalaisesta lähteestä muille kielille.

Myöhemmät muutoslokit dokumentoivat edelleen yhteensopivuuden, suorituskyvyn ja työkaluketjun muutokset.