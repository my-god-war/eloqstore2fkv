# Migrera huvudbok

`tool/fkv_ledger` Använd tree-sitter för att skanna eloqstore, kartlägga och verifiera C++-symboler och fkv
Bevis skrivs till motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Reskontradatabasen finns på `.data/motedb.mote` och är inte lagrad i databasen.

`verified` måste ha godkända bevis; `doc/progress.md` genereras av `report`.