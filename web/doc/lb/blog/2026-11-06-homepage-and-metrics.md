---
date: 2026-11-06
summary: Homepage Performance Verglach, Evaluatiounsindikator Expansioun, GitHub Säiten a Multi-Language Blogs online
---

# 2026-11-06: Haaptsäit Leeschtung Dësch, Indikator Expansioun a Säiten

Den Update vun haut konzentréiert sech op Leeschtungsbewäertungen a Sitestarten.

## Homepage Leeschtung Verglach

- D'Homepage gëtt op d'Performance Prioritéit geännert, an d'C ++/Rust Vergläichstabell gëtt direkt um éischte Bildschierm ugewisen.
- Dräi nei Sätz vun Indikatoren ginn derbäi: logesch Bandbreedung, P99.9 Schwanzverzögerung a raimlech Verstäerkung.
- Zousätzlech Rust Lafen Detailer: Scan, Fullscan, Schreifverstäerkung, Zuel vu Schreifsystemriffen an Datensynchroniséierung pro Batch.
- Markéiert `write-read` explizit als gemëschte Kaliber a Link op d'Leeschtungsdokumentatioun.

Aktuell selwescht Maschinn Baseline:

| Index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Héichpunkt RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Stockage Benotzung | 269.0 MiB | 101.4 MiB | 0.38x |

## Evaluatioun Indicateuren

Bezitt op déi gemeinsam Ausgab vu RocksDB, LevelDB, YCSB, WiredTiger an aner Benchmarks, hunn ech derbäigesat:

- Logesch liesen a schreiwen bandwidth MiB / s;
- P99.9 Schwäif Verspéidung;
- Raumverstäerkung, Schreifverstäerkung;
- Zuel vun Schreiwen System Appellen an Daten Synchroniséierung an all Batch;
- Scan / Fullscan Duerchgang;
- CPU, Säit Feeler, Kontext Wiesselt, an Prozess IO weider vun der Ressource Sidecar gesammelt ginn.

Gläichzäiteg pensionéiert`gxhash_4096B_gib_s`. Et ass e 4 KiB Single Block Hash-Mikrobenchmark deen net um KV-Motorwee ass a kee C++ Géigespiller huet.

## Richtegkeet an Ingenieur Fixen

- Fixéiert d'Schlësselraum vun `--rng cpp` liesen/scannen: Lieserkennung gëtt net méi multiplizéiert mat `key_interval` fir ze vermeiden datt 75% vun de Liesungen an ongeschriwwene Schlësselen falen.
- Fixed Standby-Wiederversuche verursaacht duerch macOS `openrsync` falsch Uerteel vu Manifestatiounen mat deemselwechten Inhalt.
- Fixéiert en Hardware Gate Thema wou QEMU virtuelle Disken als lokal SSDs falsch beurteelt goufen.

## Siten a Verëffentlechung

- GitHub Säiten aktivéiert: <https://my-god-war.github.io/eloqstore2fkv/>
- Site Routing ënnerstëtzt `/eloqstore2fkv/` Ënnerwee.
- D'Dokument gëtt amplaz vun `raw.githubusercontent.com` gelies, an den 404 verursaacht duerch fehlend `web/` am jsDelivr Wee ass fixéiert.
- Blognavigatioun an `bun run tran` Méisproocheg Iwwersetzung sinn integréiert; Dësen Artikel gëtt automatesch aus der chinesescher Quell an aner Sproochen iwwersat.

Déi spéider Changementloge wäerte weider Kompatibilitéit, Performance an Toolchain Ännerungen dokumentéieren.