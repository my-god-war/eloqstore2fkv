# Migratu liburua

`tool/fkv_ledger` Erabili tree-sitter eloqstore eskaneatzeko, mapatzeko eta C++ sinboloak eta fkv egiaztatzeko
Froga motedb-i idazten da.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Liburuko datu-basea `.data/motedb.mote` helbidean dago eta ez dago datu-basean gordetzen.

`verified` frogak izan behar ditu; `doc/progress.md` `report`k sortzen du.