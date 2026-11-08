# লেজাৰ প্ৰব্ৰজন কৰক

`tool/fkv_ledger` eloqstore স্কেন কৰিবলে, মেপ কৰিবলে আৰু C++ চিহ্ন আৰু fkv পৰীক্ষা কৰিবলে tree-sitter ব্যৱহাৰ কৰক
প্ৰমাণ motedb লৈ লিখা হয়।

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

লেজাৰ ডাটাবেইচ `.data/motedb.mote` ত আছে আৰু ডাটাবেছত সংৰক্ষণ কৰা নহয়।

`verified` পাছ প্ৰমাণ থাকিব লাগিব; `doc/progress.md` `report` দ্বাৰা সৃষ্টি কৰা হয়।