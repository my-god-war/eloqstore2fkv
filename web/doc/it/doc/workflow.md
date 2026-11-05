# Processo di verifica

## Test di compatibilità con la ruggine

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` si basa sul formato crate di `../fkv` attraverso percorsi relativi, decodifica il corpus di input e fa
Affermazioni gold standard byte per byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle compila solo il livello di formato TU e non richiede la costruzione di io_uring/S3/intera libreria.

## corpo

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Esegui `git diff --exit-code fixtures/cpp` dopo che CI si è rigenerato `fixtures/cpp` per verificare la deriva.

## linea di base

`COMPAT_BASELINE` Blocca i livelli di commit e compatibilità di eloqstore. La linea di base aggiornata deve essere nuovamente scansionata e verificata nuovamente.