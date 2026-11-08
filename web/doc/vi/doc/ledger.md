# Di chuyển sổ cái

`tool/fkv_ledger` Sử dụng tree-sitter để quét eloqstore, ánh xạ và xác minh các ký hiệu C++ và fkv
Bằng chứng được ghi vào motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Cơ sở dữ liệu sổ cái nằm ở `.data/motedb.mote` và không được lưu trữ trong cơ sở dữ liệu.

`verified` phải có bằng chứng vượt qua; `doc/progress.md` được tạo bởi `report`.