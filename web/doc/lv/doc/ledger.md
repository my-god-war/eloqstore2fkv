# Migrēt virsgrāmatu

`tool/fkv_ledger` Izmantojiet koku vietni, lai skenētu eloqstore, kartētu un pārbaudītu C++ simbolus un fkv
Pierādījumi ir rakstīti motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Virsgrāmatas datu bāze atrodas `.data/motedb.mote` un netiek glabāta datu bāzē.

`verified` jābūt sekmīgam pierādījumam; `doc/progress.md` ģenerē `report`.