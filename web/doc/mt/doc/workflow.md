# Proċess ta' verifika

## Ittestjar tal-kompatibilità tas-sadid

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` tiddependi fuq iċ-ċestun tal-format ta' `../fkv` permezz ta' mogħdijiet relattivi, jiddekodifika l-korpus tad-dħul u jagħmel
Asserzjonijiet tal-istandard tad-deheb byte b'byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle jikkompila biss is-saff tal-format TU u ma jeħtieġx io_uring / S3 / kostruzzjoni tal-librerija kollha.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Eżegwixxi `git diff --exit-code fixtures/cpp` wara li CI tirriġenera `fixtures/cpp` biex tiċċekkja għal drift.

## linja bażi

`COMPAT_BASELINE` Jillokkja l-livelli ta' kommit u kompatibilità ta' eloqstore. Il-linja bażi aġġornata għandha tiġi skannjata mill-ġdid u verifikata mill-ġdid.