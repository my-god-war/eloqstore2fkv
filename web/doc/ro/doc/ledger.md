# Migrați registrul

`tool/fkv_ledger` Utilizați tree-sitter pentru a scana eloqstore, mapa și verifica simbolurile C++ și fkv
Dovezile sunt scrise către motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Baza de date registru este la `.data/motedb.mote` și nu este stocată în baza de date.

`verified` trebuie să aibă probe; `doc/progress.md` este generat de `report`.