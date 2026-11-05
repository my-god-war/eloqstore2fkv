# لیجر منتقل کریں۔

`tool/fkv_ledger` eloqstore کو اسکین کرنے، نقشہ بنانے اور C++ علامتوں اور fkv کی تصدیق کے لیے ٹری سیٹر کا استعمال کریں
ثبوت motedb کو لکھا گیا ہے۔

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

لیجر ڈیٹا بیس `.data/motedb.mote` پر ہے اور ڈیٹا بیس میں محفوظ نہیں ہے۔

`verified` پاس ثبوت ہونا ضروری ہے؛ `doc/progress.md` `report` کے ذریعہ تیار کیا گیا ہے۔