# Hamisha leja

`tool/fkv_ledger` Tumia mti-sitter kuchanganua eloqstore, ramani na kuthibitisha alama za C++ na fkv
Ushahidi umeandikwa kwa motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Hifadhidata ya leja iko `.data/motedb.mote` na haijahifadhiwa kwenye hifadhidata.

`verified` lazima iwe na ushahidi wa kupita; `doc/progress.md` imetolewa na `report`.