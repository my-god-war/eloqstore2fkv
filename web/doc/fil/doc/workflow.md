# Proseso ng pagpapatunay

## Pagsubok sa pagiging tugma ng kalawang

```bash
cargo nextest run -p fkv_compat
```

Umaasa ang `fkv_compat` sa format na crate ng `../fkv` sa pamamagitan ng mga relatibong path, nagde-decode ng input corpus at ginagawa
Byte-by-byte na pamantayang ginto.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Kino-compile lang ng Oracle ang format na layer na TU at hindi nangangailangan ng io_uring / S3 / buong pagtatayo ng library.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

I-execute ang `git diff --exit-code fixtures/cpp` pagkatapos ma-regenerate ng CI ang `fixtures/cpp` para tingnan kung may drift.

## baseline

`COMPAT_BASELINE` Nila-lock ang eloqstore commit at mga antas ng compatibility. Ang na-upgrade na baseline ay dapat na muling i-scan at i-verify.