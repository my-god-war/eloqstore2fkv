# Grootboek migreren

`tool/fkv_ledger` Gebruik tree-sitter om eloqstore te scannen, C++-symbolen en fkv in kaart te brengen en te verifiëren
Bewijs wordt naar motedb geschreven.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

De grootboekdatabase bevindt zich op `.data/motedb.mote` en is niet opgeslagen in de database.

`verified` moet een pasbewijs hebben; `doc/progress.md` wordt gegenereerd door `report`.