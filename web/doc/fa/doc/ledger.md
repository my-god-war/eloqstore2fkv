# مهاجرت دفتر

`tool/fkv_ledger` از tree-sitter برای اسکن eloqstore، نقشه برداری و تأیید نمادهای C++ و fkv استفاده کنید.
شواهد به motedb نوشته شده است.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

پایگاه داده دفتر کل در `.data/motedb.mote` است و در پایگاه داده ذخیره نمی شود.

`verified` باید مدرک معتبر داشته باشد. `doc/progress.md` توسط `report` ایجاد می‌شود.