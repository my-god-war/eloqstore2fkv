# Pagbalhin sa ledger

`tool/fkv_ledger` Gamita ang tree-sitter aron ma-scan ang eloqstore, mapa ug i-verify ang mga simbolo sa C++ ug fkv
Ang ebidensya gisulat sa motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Ang ledger database anaa sa `.data/motedb.mote` ug wala gitipigan sa database.

`verified` kinahanglan adunay pass nga ebidensya; `doc/progress.md` gihimo ni `report`.