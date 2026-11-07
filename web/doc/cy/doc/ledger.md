# Cyfriflyfr mudo

`tool/fkv_ledger` Defnyddiwch warchodwr coed i sganio eloqstore, mapio a gwirio symbolau C++ a fkv
Ysgrifenir tystiolaeth i motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Mae cronfa ddata'r cyfriflyfr yn `.data/motedb.mote` ac nid yw wedi'i storio yn y gronfa ddata.

rhaid bod `verified` wedi pasio tystiolaeth; `doc/progress.md` yn cael ei gynhyrchu gan `report`.