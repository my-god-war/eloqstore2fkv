# Verifikasie proses

## Roes verenigbaarheid toets

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` maak staat op die formaatkrat van `../fkv` deur relatiewe paaie, dekodeer die invoerkorpus en doen
Byte-vir-byte goue standaard stellings.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle stel slegs die formaatlaag TU saam en vereis nie io_uring / S3 / hele biblioteekkonstruksie nie.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Voer `git diff --exit-code fixtures/cpp` uit nadat CI `fixtures/cpp` herstel het om te kyk vir wegdrywing.

## basislyn

`COMPAT_BASELINE` Sluit eloqstore-toewyding en verenigbaarheidsvlakke. Die opgegradeerde basislyn moet herskandeer en herverifieer word.