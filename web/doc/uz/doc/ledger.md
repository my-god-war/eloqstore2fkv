# Ko'chirish kitobi

`tool/fkv_ledger` Eloqstore-ni skanerlash, C++ belgilari va fkv-ni xaritalash va tekshirish uchun daraxtzordan foydalaning
Dalillar motedbga yozilgan.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Buxgalteriya hisobi ma'lumotlar bazasi `.data/motedb.mote`da joylashgan va ma'lumotlar bazasida saqlanmaydi.

`verified` o'tish dalillariga ega bo'lishi kerak; `doc/progress.md` `report` tomonidan yaratilgan.