# Verifikationsproces

## Rustkompatibilitetstest

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` er afhængig af formatkassen `../fkv` gennem relative stier, afkoder inputkorpuset og gør
Byte-for-byte guldstandard påstande.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompilerer kun formatlaget TU og kræver ikke io_uring / S3 / hele bibliotekskonstruktion.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Udfør `git diff --exit-code fixtures/cpp` efter CI regenererer `fixtures/cpp` for at kontrollere for drift.

## baseline

`COMPAT_BASELINE` Låser eloqstore commit- og kompatibilitetsniveauer. Den opgraderede baseline skal genscannes og bekræftes igen.