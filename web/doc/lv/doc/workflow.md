# Verifikācijas process

## Rūsas saderības pārbaude

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` paļaujas uz `../fkv` formāta kasti, izmantojot relatīvos ceļus, atkodē ievades korpusu un veic
Zelta standarta apgalvojumi pa baitam.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle apkopo tikai formāta slāni TU un neprasa io_uring / S3 / visu bibliotēkas izveidi.

## korpuss

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Izpildiet `git diff --exit-code fixtures/cpp` pēc CI reģenerācijas `fixtures/cpp`, lai pārbaudītu novirzi.

## bāzes līnija

`COMPAT_BASELINE` Bloķē eloqstore saistību un saderības līmeņus. Jauninātā bāzes līnija ir atkārtoti jāskenē un atkārtoti jāpārbauda.