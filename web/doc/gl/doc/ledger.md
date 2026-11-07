# Migrar libro

`tool/fkv_ledger` Usa tree-sitter para escanear eloqstore, mapear e verificar símbolos C++ e fkv
Escríbese a evidencia a motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

A base de datos do libro maior está en `.data/motedb.mote` e non está almacenada na base de datos.

`verified` debe ter probas de aprobación; `doc/progress.md` é xerado por `report`.