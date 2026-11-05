# Migració del llibre major

`tool/fkv_ledger` Utilitzeu tree-sitter per escanejar eloqstore, mapejar i verificar símbols C++ i fkv
L'evidència s'escriu a motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

La base de dades del llibre major es troba a `.data/motedb.mote` i no s'emmagatzema a la base de dades.

`verified` ha de tenir proves de pas; `doc/progress.md` el genera `report`.