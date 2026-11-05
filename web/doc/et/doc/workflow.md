# Kinnitusprotsess

## Rooste ühilduvuse testimine

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` tugineb suhteliste teede kaudu `../fkv` vormingukastile, dekodeerib sisendkorpuse ja teeb
Bait-baidi kaupa kuldstandardi väited.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompileerib ainult vormingukihi TU ja ei nõua io_uring / S3 / kogu teegi ehitamist.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Triivimise kontrollimiseks käivitage `git diff --exit-code fixtures/cpp` pärast CI regenereerimist `fixtures/cpp`.

## baasjoon

`COMPAT_BASELINE` Lukustab eloqstore'i kohustus- ja ühilduvustasemed. Täiendatud baasväärtus tuleb uuesti skannida ja uuesti kinnitada.