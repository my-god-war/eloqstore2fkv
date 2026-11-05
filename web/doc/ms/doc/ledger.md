# Berhijrah lejar

`tool/fkv_ledger` Gunakan tree-sitter untuk mengimbas eloqstore, memetakan dan mengesahkan simbol C++ dan fkv
Bukti ditulis kepada motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Pangkalan data lejar berada di `.data/motedb.mote` dan tidak disimpan dalam pangkalan data.

`verified` mesti mempunyai bukti pas; `doc/progress.md` dijana oleh `report`.