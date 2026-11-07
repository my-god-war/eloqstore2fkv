# Migrasi ledger

`tool/fkv_ledger` Gunakake tree-sitter kanggo mindhai eloqstore, map lan verifikasi simbol C++ lan fkv
Bukti ditulis kanggo motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Basis data ledger ana ing `.data/motedb.mote` lan ora disimpen ing basis data.

`verified` kudu duwe bukti pass; `doc/progress.md` digawe dening `report`.