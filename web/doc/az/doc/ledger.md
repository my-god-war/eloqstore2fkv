# Kitabı köçürün

`tool/fkv_ledger` Eloqstore-u skan etmək, C++ simvollarını və fkv-ni xəritələşdirmək və yoxlamaq üçün ağac baxıcısından istifadə edin
Sübut motedb-ə yazılır.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Defter verilənlər bazası `.data/motedb.mote`-dədir və verilənlər bazasında saxlanmır.

`verified` keçid sübutu olmalıdır; `doc/progress.md` `report` tərəfindən yaradılıb.