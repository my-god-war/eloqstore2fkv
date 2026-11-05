# Migrar libro mayor

`tool/fkv_ledger` Utilice tree-sitter para escanear eloqstore, mapear y verificar símbolos C++ y fkv
La evidencia se escribe en moteb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

La base de datos del libro mayor está en `.data/motedb.mote` y no está almacenada en la base de datos.

`verified` debe tener pruebas de aprobación; `doc/progress.md` es generado por `report`.