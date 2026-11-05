# העבר ספר חשבונות

`tool/fkv_ledger` השתמש ב-tree-sitter כדי לסרוק את eloqstore, למפות ולאמת סמלי C++ ו-fkv
עדות נכתבת ל-motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

מסד הנתונים של ספר החשבונות נמצא ב-`.data/motedb.mote` ואינו מאוחסן במסד הנתונים.

`verified` חייבת להיות הוכחה עוברת; `doc/progress.md` נוצר על ידי `report`.