# Көшіру кітапшасы

`tool/fkv_ledger` eloqstore сканерлеу, C++ символдары мен fkv картасын картаға түсіру және тексеру үшін ағаш күтушісін пайдаланыңыз.
Дәлелдер motedb сайтына жазылған.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Бухгалтерлік кітаптың дерекқоры `.data/motedb.mote` орнында және дерекқорда сақталмайды.

`verified` өту дәлелі болуы керек; `doc/progress.md` `report` арқылы жасалады.