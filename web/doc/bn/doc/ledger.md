# মাইগ্রেট লেজার

`tool/fkv_ledger` eloqstore স্ক্যান করতে, C++ চিহ্ন এবং fkv মানচিত্র এবং যাচাই করতে ট্রি-সিটার ব্যবহার করুন
প্রমাণ motedb লিখিত.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

লেজার ডাটাবেস `.data/motedb.mote` এ আছে এবং ডাটাবেসে সংরক্ষিত নেই।

`verified` পাস প্রমাণ থাকতে হবে; `doc/progress.md` `report` দ্বারা তৈরি হয়৷