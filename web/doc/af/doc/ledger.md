# Migreer grootboek

`tool/fkv_ledger` Gebruik tree-sitter om eloqstore te skandeer, te karteer en C++-simbole en fkv te verifieer
Bewyse word aan motedb geskryf.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Die grootboekdatabasis is by `.data/motedb.mote` en word nie in die databasis gestoor nie.

`verified` moet slaagbewyse hê; `doc/progress.md` word gegenereer deur `report`.