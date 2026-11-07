# Hauptbuch migrieren

`tool/fkv_ledger` Verwenden Sie Tree-Sitter, um eloqstore zu scannen, C++-Symbole und FKV zuzuordnen und zu überprüfen
Beweise werden an motedb geschrieben.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Die Ledger-Datenbank befindet sich unter `.data/motedb.mote` und wird nicht in der Datenbank gespeichert.

`verified` muss über einen bestandenen Nachweis verfügen; `doc/progress.md` wird von `report` generiert.