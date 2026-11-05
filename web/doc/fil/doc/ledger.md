# Ilipat ang ledger

`tool/fkv_ledger` Gumamit ng tree-sitter para i-scan ang eloqstore, mapa at i-verify ang mga simbolo at fkv ng C++
Ang ebidensya ay isinulat sa motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Ang database ng ledger ay nasa `.data/motedb.mote` at hindi nakaimbak sa database.

Ang `verified` ay dapat mayroong pass evidence; Ang `doc/progress.md` ay nabuo ng `report`.