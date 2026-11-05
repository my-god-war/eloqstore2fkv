# Миграција књиге

<б ид="1"> Користите трее-ситтер да скенирате елоксторе, мапирате и верификујете Ц++ симболе и фкв
Докази су писани мотедб-у.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

База података главне књиге је на <б ид="1"> и није ускладиштена у бази података.

<б ид="1"> мора имати доказ о пролазу; <б ид="2"> генерише <б ид="3">.