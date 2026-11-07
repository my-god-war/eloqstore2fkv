# Emigra l-ktieb

`tool/fkv_ledger` Uża tree-sitter biex tiskennja l-eloqstore, timmappa u tivverifika simboli C++ u fkv
L-evidenza tinkiteb lil motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Id-database tar-reġistru tinsab f'`.data/motedb.mote` u mhix maħżuna fid-database.

`verified` irid ikollu evidenza ta' pass; `doc/progress.md` huwa ġġenerat minn `report`.