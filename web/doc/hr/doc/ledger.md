# Migracija knjige

`tool/fkv_ledger` Koristite tree-sitter za skeniranje eloqstorea, mapiranje i provjeru C++ simbola i fkv
Dokazi se pišu na motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Baza podataka glavne knjige nalazi se na `.data/motedb.mote` i nije pohranjena u bazi podataka.

`verified` mora imati dokaz o prolaznosti; `doc/progress.md` generira `report`.