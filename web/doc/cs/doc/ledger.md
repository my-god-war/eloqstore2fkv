# Přenést účetní knihu

`tool/fkv_ledger` Použijte tree-sitter ke skenování eloqstore, mapování a ověřování C++ symbolů a fkv
Důkazy jsou zapsány do motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Databáze hlavní knihy je na `.data/motedb.mote` a není uložena v databázi.

`verified` musí mít průkaz totožnosti; `doc/progress.md` je generováno `report`.