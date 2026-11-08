# Migrasikan buku besar

`tool/fkv_ledger` Gunakan tree-sitter untuk memindai eloqstore, memetakan dan memverifikasi simbol C++ dan fkv
Bukti ditulis ke motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Basis data buku besar berada di `.data/motedb.mote` dan tidak disimpan dalam basis data.

`verified` harus mempunyai bukti yang sah; `doc/progress.md` dihasilkan oleh `report`.