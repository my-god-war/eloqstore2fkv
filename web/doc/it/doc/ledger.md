# Migrare il registro

`tool/fkv_ledger` Utilizza tree-sitter per scansionare eloqstore, mappare e verificare simboli C++ e fkv
Le prove sono scritte su motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Il database del libro mastro si trova in `.data/motedb.mote` e non è archiviato nel database.

`verified` deve avere prove sufficienti; `doc/progress.md` è generato da `report`.