# لڏپلاڻ وارو ليجر

`tool/fkv_ledger` eloqstore اسڪين ڪرڻ لاءِ ٽري-سيٽر استعمال ڪريو، نقشو ۽ تصديق ڪريو C++ علامتون ۽ fkv
ثبوت motedb ڏانهن لکيل آهي.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ليجر ڊيٽابيس `.data/motedb.mote` تي آهي ۽ ڊيٽابيس ۾ محفوظ نه آهي.

`verified` پاس ثبوت هجڻ گهرجي؛ `doc/progress.md` ٺاهيل آهي `report` پاران.