# Overfør hovedbog

`tool/fkv_ledger` Brug tree-sitter til at scanne eloqstore, kortlægge og verificere C++ symboler og fkv
Bevis skrives til motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Finansdatabasen er på `.data/motedb.mote` og er ikke gemt i databasen.

`verified` skal have bestået bevis; `doc/progress.md` er genereret af `report`.