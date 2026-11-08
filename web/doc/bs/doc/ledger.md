# Migracija knjige

`tool/fkv_ledger` Koristite tree-sitter da skenirate eloqstore, mapirate i provjerite C++ simbole i fkv
Dokazi su pisani motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Baza podataka glavne knjige je na `.data/motedb.mote` i nije pohranjena u bazi podataka.

`verified` mora imati dokaz o prolazu; `doc/progress.md` generira `report`.