# Przeprowadź migrację księgi

`tool/fkv_ledger` Użyj narzędzia Tree-Sitter do skanowania eloqstore, mapowania i weryfikacji symboli C++ oraz fkv
Dowody są zapisywane w motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Baza danych księgi znajduje się pod adresem `.data/motedb.mote` i nie jest przechowywana w bazie danych.

`verified` musi mieć dowód; `doc/progress.md` jest generowane przez `report`.