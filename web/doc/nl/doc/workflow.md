# Verificatieproces

## Testen van roestcompatibiliteit

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` vertrouwt op het formaatkrat van `../fkv` via relatieve paden, decodeert het invoercorpus en doet
Byte-voor-byte gouden standaardbeweringen.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle compileert alleen de formaatlaag TU en vereist geen io_uring / S3 / volledige bibliotheekconstructie.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Voer `git diff --exit-code fixtures/cpp` uit nadat CI `fixtures/cpp` opnieuw heeft gegenereerd om te controleren op drift.

## basislijn

`COMPAT_BASELINE` Vergrendelt eloqstore commit- en compatibiliteitsniveaus. De geüpgradede basislijn moet opnieuw worden gescand en opnieuw worden geverifieerd.