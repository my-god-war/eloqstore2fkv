# Proseso sa pag-verify

## Pagsulay sa pagkaangay sa taya

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` nagsalig sa format nga crate sa `../fkv` pinaagi sa mga relatibong agianan, nag-decode sa input corpus ug naghimo
Byte-by-byte nga bulawan nga standard assertions.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Ang Oracle nag-compile lang sa format nga layer TU ug wala magkinahanglan og io_uring / S3 / tibuok nga pagtukod sa librarya.

## lawas

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

I-execute ang `git diff --exit-code fixtures/cpp` human ma-regenerate ang CI `fixtures/cpp` aron masusi ang drift.

## baseline

`COMPAT_BASELINE` I-lock ang eloqstore commit ug compatibility nga lebel. Ang gi-upgrade nga baseline kinahanglan nga i-scan pag-usab ug i-verify pag-usab.