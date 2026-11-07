# Verification process

## Rust compatibility testing

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` relies on the format crate of `../fkv` through relative paths, decodes the input corpus and does
Byte-by-byte gold standard assertions.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle only compiles the format layer TU and does not require io_uring / S3 / entire library construction.

## Corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Execute `git diff --exit-code fixtures/cpp` after CI regenerates `fixtures/cpp` to check for drift.

## Baseline

`COMPAT_BASELINE` Locks eloqstore commit and compatibility levels. The upgraded baseline must be rescanned and reverified.