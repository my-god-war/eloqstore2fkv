# Defteri taşı

`tool/fkv_ledger` Eloqstore'u taramak, C++ sembollerini ve fkv'yi eşlemek ve doğrulamak için ağaç bakıcısını kullanın
Kanıt motedb'ye yazılır.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Genel muhasebe veritabanı `.data/motedb.mote` konumundadır ve veritabanında saklanmaz.

`verified` geçerli kanıta sahip olmalıdır; `doc/progress.md`, `report` tarafından oluşturulur.