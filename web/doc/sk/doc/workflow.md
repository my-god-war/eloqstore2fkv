# Proces overovania

## Testovanie kompatibility s hrdzou

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` sa spolieha na formát prepravky `../fkv` prostredníctvom relatívnych ciest, dekóduje vstupný korpus a robí
Tvrdenia zlatého štandardu bajt po byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle iba kompiluje formátovú vrstvu TU a nevyžaduje io_uring / S3 / celú konštrukciu knižnice.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Vykonajte `git diff --exit-code fixtures/cpp` po regenerácii CI `fixtures/cpp`, aby ste skontrolovali posun.

## základná línia

`COMPAT_BASELINE` Uzamyká úroveň odovzdania a kompatibility eloqstore. Aktualizovaná základná línia sa musí znova naskenovať a znova overiť.