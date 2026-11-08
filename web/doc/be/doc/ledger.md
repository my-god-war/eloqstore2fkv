# Перанесці кнігу

`tool/fkv_ledger` Выкарыстоўвайце Tree-sitter для сканавання eloqstore, адлюстравання і праверкі сімвалаў C++ і fkv
Доказы запісваюцца ў motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

База даных кнігі знаходзіцца па адрасе `.data/motedb.mote` і не захоўваецца ў базе даных.

`verified` неабходна мець доказы пропуску; `doc/progress.md` генеруецца `report`.