# Migrar razão

`tool/fkv_ledger` Use tree-sitter para escanear eloqstore, mapear e verificar símbolos C++ e fkv
A evidência é escrita em motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

O banco de dados contábil está em `.data/motedb.mote` e não está armazenado no banco de dados.

`verified` deve ter provas de aprovação; `doc/progress.md` é gerado por `report`.