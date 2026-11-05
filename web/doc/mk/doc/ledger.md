# Мигрирајте ја книгата

`tool/fkv_ledger` Користете дрво-ситер за скенирање на eloqstore, мапирање и потврдување на C++ симболи и fkv
Доказите се напишани на motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Базата на податоци на книгата е на `.data/motedb.mote` и не е зачувана во базата на податоци.

`verified` мора да има проодни докази; `doc/progress.md` е генериран од `report`.