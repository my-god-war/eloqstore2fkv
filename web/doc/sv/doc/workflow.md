# Verifieringsprocess

## Testning av rostkompatibilitet

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` förlitar sig på formatlådan för `../fkv` genom relativa sökvägar, avkodar indatakorpusen och gör
Byte-för-byte guldstandard påståenden.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompilerar bara formatlagret TU och kräver inte io_uring / S3 / hela bibliotekskonstruktionen.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Kör `git diff --exit-code fixtures/cpp` efter att CI regenererat `fixtures/cpp` för att kontrollera om det inte finns någon drift.

## baslinje

`COMPAT_BASELINE` Låser eloqstore commit- och kompatibilitetsnivåer. Den uppgraderade baslinjen måste skannas om och verifieras på nytt.