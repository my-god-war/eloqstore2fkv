# Ellenőrzési folyamat

## Rozsdakompatibilitási vizsgálat

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` a `../fkv` formátum ládájára támaszkodik a relatív útvonalakon keresztül, dekódolja a bemeneti korpuszt és
Byte-byte gold standard állítások.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Az Oracle csak a TU formátumréteget fordítja le, és nem igényel io_uring / S3 / teljes könyvtári felépítést.

## korpusz

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Hajtsa végre a `git diff --exit-code fixtures/cpp` parancsot, miután a CI újragenerált `fixtures/cpp` az elsodródás ellenőrzéséhez.

## alapvonal

`COMPAT_BASELINE` Zárolja az eloqstore véglegesítési és kompatibilitási szintjeit. A frissített alapvonalat újra be kell vizsgálni, és újra ellenőrizni kell.