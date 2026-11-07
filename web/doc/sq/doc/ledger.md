# Migrimi i librit

`tool/fkv_ledger` Përdor pemë-sitter për të skanuar eloqstore, për të hartuar dhe verifikuar simbolet e C++ dhe fkv
Dëshmia është shkruar në motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Baza e të dhënave të librit është në `.data/motedb.mote` dhe nuk ruhet në bazën e të dhënave.

`verified` duhet të ketë dëshmi të kaluar; `doc/progress.md` është krijuar nga `report`.