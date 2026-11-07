# Főkönyv migrálása

`tool/fkv_ledger` Használja a tree-sittert az eloqstore átvizsgálására, a C++ szimbólumok és fkv feltérképezésére és ellenőrzésére
A bizonyítékokat a motedb-nek írják.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

A főkönyvi adatbázis a következő helyen található: `.data/motedb.mote`, és nincs az adatbázisban tárolva.

`verified` megfelelő bizonyítékkal kell rendelkeznie; A `doc/progress.md` `report` által generált.