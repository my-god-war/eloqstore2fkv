# 台帳の移行

`tool/fkv_ledger`tree-sitter を使用して eloqstore をスキャンし、C++ シンボルと fkv をマッピングおよび検証します
証拠は motedb に書き込まれます。

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

台帳データベースは`.data/motedb.mote`にあり、データベースには保存されません。

`verified` 合格の証拠が必要です。 `doc/progress.md` は `report` によって生成されます。