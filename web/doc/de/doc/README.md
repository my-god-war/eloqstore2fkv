# eloqstore2fkv

Migrations- und Kompatibilitätsüberprüfungs-Warehouse von eloqstore zu fkv.

Das Haupt-fkv-Repository bleibt reines Rust und wird unabhängig erstellt; Dieses Repository enthält die C++-Referenzimplementierung Oracle, den bidirektionalen Bytekorpus,
Migrieren Sie Hauptbücher und historische Prüfaufzeichnungen und überprüfen Sie CI.

## Inhalt

- C++ Oracle: Eloqstore-Formatebene verknüpfen und Seite/Index/Überlauf/Manifest/ generieren
  Korpus komprimieren.
- Bidirektionaler Korpus: C++-Schreiben → Rust-Lesen, Rust-Schreiben → C++-Lesen.
- `fkv_compat`: Rust-seitiger Kompatibilitätstest, basierend auf der Formatkiste des fkv-Warehouses.
- `fkv_ledger`: Eloqstore scannen, Migrations-Ledger aufzeichnen, Korpus generieren und überprüfen.
- Leistungsvergleich: Vergleichen Sie FKV- und C++-Baselines unter derselben Arbeitslast.

## Eingang

- [Verifizierungsprozess](workflow.md)
- [C++ oracle](oracle.md)
- [Hauptbuch migrieren](ledger.md)
- [Leistung](performance.md)
- [Überprüfen Sie die Aufzeichnungen](issues.md)