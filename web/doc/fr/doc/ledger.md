# Migrer le grand livre

`tool/fkv_ledger` Utilisez tree-sitter pour analyser eloqstore, mapper et vérifier les symboles C++ et fkv
Les preuves sont écrites sur motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

La base de données du grand livre se trouve à `.data/motedb.mote` et n'est pas stockée dans la base de données.

`verified` doit avoir une preuve de réussite ; `doc/progress.md` est généré par `report`.