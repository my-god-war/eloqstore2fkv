# Overfør hovedbok

`tool/fkv_ledger` Bruk tree-sitter til å skanne eloqstore, kartlegge og verifisere C++-symboler og fkv
Bevis skrives til motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Reskontrodatabasen er på `.data/motedb.mote` og er ikke lagret i databasen.

`verified` må ha bestått bevis; `doc/progress.md` er generert av `report`.