# Տեղափոխել մատյան

`tool/fkv_ledger` Օգտագործեք ծառի տեղադրիչը eloqstore-ը սկանավորելու, քարտեզագրելու և ստուգելու C++ սիմվոլները և fkv-ն
Ապացույցները գրված են motedb-ում:

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Գրառման տվյալների բազան գտնվում է `.data/motedb.mote` հասցեում և չի պահվում տվյալների բազայում:

`verified` պետք է ունենա անցած ապացույց. `doc/progress.md`-ը ստեղծվում է `report`-ի կողմից: