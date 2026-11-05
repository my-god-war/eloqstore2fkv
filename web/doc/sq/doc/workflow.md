# Procesi i verifikimit

## Testimi i përputhshmërisë së ndryshkut

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` mbështetet në arkën e formatit të `../fkv` përmes shtigjeve relative, dekodon korpusin e hyrjes dhe bën
Pohimet e standardit të arit byte-pas-byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle përpilon vetëm shtresën e formatit TU dhe nuk kërkon io_uring / S3 / të gjithë ndërtimin e bibliotekës.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Ekzekutoni `git diff --exit-code fixtures/cpp` pasi CI të rigjenerohet `fixtures/cpp` për të kontrolluar për drift.

## bazë

`COMPAT_BASELINE` Locks eloqstore angazhohen dhe nivelet e pajtueshmërisë. Vija bazë e përmirësuar duhet të skanohet dhe të riverifikohet.