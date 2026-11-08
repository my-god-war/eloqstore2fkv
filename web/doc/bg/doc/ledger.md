# Мигриране на книга

`tool/fkv_ledger` Използвайте tree-sitter за сканиране на eloqstore, карта и проверка на C++ символи и fkv
Доказателствата се записват в motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Базата данни на главната книга е `.data/motedb.mote` и не се съхранява в базата данни.

`verified` трябва да има доказателство за преминаване; `doc/progress.md` се генерира от `report`.