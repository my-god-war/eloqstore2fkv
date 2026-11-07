# Pearaamatu migreerimine

`tool/fkv_ledger` Kasutage puuhoidjat eloqstore'i skannimiseks, kaardistamiseks ja C++ sümbolite ja fkv kontrollimiseks
Tõendid kirjutatakse motedb-le.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Pearaamatu andmebaas asub aadressil `.data/motedb.mote` ja seda andmebaasi ei salvestata.

`verified` peab olema läbinud tõend; `doc/progress.md` genereerib `report`.