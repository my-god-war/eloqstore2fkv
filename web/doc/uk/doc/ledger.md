# Перенести книгу

`tool/fkv_ledger` Використовуйте Tree-sitter для сканування eloqstore, зіставлення та перевірки символів C++ і fkv
Докази записуються в motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

База даних книги знаходиться за адресою `.data/motedb.mote` і не зберігається в базі даних.

`verified` повинні мати підтвердження пропуску; `doc/progress.md` генерується `report`.