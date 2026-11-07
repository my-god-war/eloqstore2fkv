---
date: 2026-11-06
summary: Leistungsvergleich der Homepage, Erweiterung der Bewertungsindikatoren, GitHub-Seiten und mehrsprachige Blogs online
---

# 06.11.2026: Leistungstabelle der Startseite, Indikatorerweiterung und Seiten

Das heutige Update konzentriert sich auf Leistungsüberprüfungen und Website-Starts.

## Leistungsvergleich der Homepage

- Die Startseite wird auf Leistungspriorität umgestellt und die C++/Rust-Vergleichstabelle wird direkt auf dem ersten Bildschirm angezeigt.
- Drei neue Indikatorensätze werden hinzugefügt: logische Bandbreite, P99.9-Tail-Delay und räumliche Verstärkung.
- Rust-Laufdetails hinzugefügt: Scan, Fullscan, Schreibverstärkung, Anzahl der Schreibsystemaufrufe und Datensynchronisierung pro Stapel.
- Markieren Sie `write-read` ausdrücklich als gemischtes Kaliber und verlinken Sie auf die Leistungsdokumentation.

Aktuelle Basislinie für die gleiche Maschine:

| Index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Spitzen-RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Speichernutzung | 269.0 MiB | 101.4 MiB | 0.38x |

## Bewertungsindikatoren

Bezugnehmend auf die gemeinsame Ausgabe von RocksDB, LevelDB, YCSB, WiredTiger und anderen Benchmarks habe ich hinzugefügt:

- Logische Lese- und Schreibbandbreite MiB/s;
- P99.9 Heckverzögerung;
- Raumverstärkung, Schreibverstärkung;
- Anzahl der Schreibsystemaufrufe und Datensynchronisation in jedem Stapel;
- Scan-/Fullscan-Durchsatz;
- CPU, Seitenfehler, Kontextwechsel und Prozess-E/A werden weiterhin vom Ressourcen-Sidecar erfasst.

Gleichzeitig im Ruhestand`gxhash_4096B_gib_s`. Es handelt sich um einen 4-KiB-Einzelblock-Hash-Mikrobenchmark, der sich nicht auf dem KV-Engine-Pfad befindet und kein C++-Gegenstück hat.

## Korrektheit und technische Korrekturen

- Korrigieren Sie den Schlüsselraum von `--rng cpp` Lesen/Scannen: Die Leseerkennung wird nicht mehr mit `key_interval` multipliziert, um zu verhindern, dass 75 % der Lesevorgänge in ungeschriebene Schlüssel fallen.
- Es wurden Standby-Wiederholungsversuche behoben, die durch eine Fehleinschätzung von Manifesten mit demselben Inhalt durch macOS `openrsync` verursacht wurden.
- Es wurde ein Hardware-Gate-Problem behoben, bei dem virtuelle QEMU-Festplatten fälschlicherweise als lokale SSDs eingeschätzt wurden.

## Websites und Veröffentlichung

- GitHub-Seiten aktiviert: <https://my-god-war.github.io/eloqstore2fkv/>
- Site-Routing unterstützt `/eloqstore2fkv/` Unterpfade.
- Das Dokument wird stattdessen von `raw.githubusercontent.com` gelesen und der 404-Fehler, der durch das Fehlen von `web/` im jsDelivr-Pfad verursacht wird, wurde behoben.
- Blog-Navigation und `bun run tran` mehrsprachige Übersetzung sind integriert; Dieser Artikel wurde automatisch aus der chinesischen Quelle in andere Sprachen übersetzt.

Nachfolgende Änderungsprotokolle werden weiterhin Kompatibilitäts-, Leistungs- und Toolchain-Änderungen dokumentieren.