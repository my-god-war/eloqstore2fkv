# Verifizierungsprozess

## Rostverträglichkeitsprüfung

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` stützt sich auf die Formatkiste von `../fkv` über relative Pfade, dekodiert den Eingabekorpus und tut dies
Byteweise Goldstandard-Behauptungen.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompiliert nur die Formatschicht TU und erfordert keine io_uring/S3/gesamte Bibliothekskonstruktion.

## Korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Führen Sie `git diff --exit-code fixtures/cpp` aus, nachdem CI `fixtures/cpp` neu generiert hat, um auf Drift zu prüfen.

## Grundlinie

`COMPAT_BASELINE` Sperrt eloqstore-Commit- und Kompatibilitätsstufen. Die aktualisierte Baseline muss erneut gescannt und überprüft werden.