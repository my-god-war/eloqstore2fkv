# Migrovať účtovnú knihu

`tool/fkv_ledger` Použite tree-sitter na skenovanie eloqstore, mapovanie a overovanie C++ symbolov a fkv
Dôkazy sú napísané na motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Databáza účtovnej knihy je na `.data/motedb.mote` a nie je uložená v databáze.

`verified` musí mať overený dôkaz; `doc/progress.md` generuje `report`.