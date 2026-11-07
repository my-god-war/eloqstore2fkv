# Migréieren Ledger

`tool/fkv_ledger` Benotzt Bam-Sitter fir eloqstore ze scannen, ze kartéieren an ze verifizéieren C++ Symboler a fkv
Beweiser geschriwwen motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

D'Ledger-Datebank ass op `.data/motedb.mote` a gëtt net an der Datebank gespäichert.

`verified` muss Passbeweis hunn; `doc/progress.md` gëtt vun `report` generéiert.