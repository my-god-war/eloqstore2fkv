# Preseli knjigo

`tool/fkv_ledger` Uporabite tree-sitter za skeniranje eloqstore, preslikavo in preverjanje simbolov C++ in fkv
Dokazi so zapisani v motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Baza podatkov glavne knjige je na `.data/motedb.mote` in ni shranjena v bazi podatkov.

`verified` mora imeti dokazilo o prehodu; `doc/progress.md` ustvari `report`.