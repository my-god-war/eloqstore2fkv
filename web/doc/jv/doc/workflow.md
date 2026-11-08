# Proses verifikasi

## Tes kompatibilitas karat

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` gumantung ing format peti `../fkv` liwat path relatif, decodes korpus input lan nindakake
Pernyataan standar emas bait-by-byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle mung nyusun lapisan format TU lan ora mbutuhake io_uring / S3 / kabeh konstruksi perpustakaan.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Jalanake `git diff --exit-code fixtures/cpp` sawise CI regenerasi `fixtures/cpp` kanggo mriksa drift.

## dhasar

`COMPAT_BASELINE` Ngunci tingkat komit lan kompatibilitas eloqstore. Baseline sing wis ditingkatake kudu dipindai maneh lan diverifikasi maneh.