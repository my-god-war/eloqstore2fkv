# Proces de verificare

## Testare de compatibilitate cu rugina

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` se bazează pe cutia de format a `../fkv` prin căi relative, decodifică corpus de intrare și nu
Afirmații standard de aur octeți cu octeți.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle compilează doar stratul de format TU și nu necesită io_uring / S3 / construcția întregii biblioteci.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Executați `git diff --exit-code fixtures/cpp` după ce CI regenerează `fixtures/cpp` pentru a verifica deviația.

## linia de bază

`COMPAT_BASELINE` Blocează nivelurile de confirmare și compatibilitate eloqstore. Linia de bază actualizată trebuie să fie rescanată și verificată.