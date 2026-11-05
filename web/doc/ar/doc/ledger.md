# ترحيل دفتر الأستاذ

`tool/fkv_ledger` استخدم Tree-sitter لمسح eloqstore وتعيين رموز C++ وfkv والتحقق منها
يتم كتابة الأدلة إلى motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

قاعدة بيانات دفتر الأستاذ موجودة في `.data/motedb.mote` ولا يتم تخزينها في قاعدة البيانات.

`verified` يجب أن يكون لديه دليل مرور؛ تم إنشاء `doc/progress.md` بواسطة `report`.