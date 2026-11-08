# Перенос реестра

`tool/fkv_ledger` Используйте Tree-sitter для сканирования eloqstore, сопоставления и проверки символов C++ и fkv.
Доказательства записываются в motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

База данных реестра находится по адресу `.data/motedb.mote` и не хранится в базе данных.

`verified` должно иметь подтверждение прохождения; `doc/progress.md` создается `report`.