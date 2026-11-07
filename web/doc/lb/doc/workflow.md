# Verifikatiounsprozess

## Rust Kompatibilitéit Testen

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` hänkt op de Formatkrat vun `../fkv` duerch relativ Weeër, dekodéiert den Input Corpus an mécht
Byte-by-byte Gold Standard Behaaptungen.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompiléiert nëmmen d'Formatschicht TU a brauch net io_uring / S3 / ganz Bibliothéikskonstruktioun.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Ausféieren `git diff --exit-code fixtures/cpp` nodeems CI regeneréiert `fixtures/cpp` fir no Drift ze kontrolléieren.

## baseline

`COMPAT_BASELINE` Späert eloqstore engagéieren an Onbedenklechkeet Niveauen. Déi aktualiséiert Baseline muss nei gescannt an nei verifizéiert ginn.