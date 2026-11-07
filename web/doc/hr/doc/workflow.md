# Proces provjere

## Ispitivanje kompatibilnosti s hrđom

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` oslanja se na okvir formata `../fkv` kroz relativne staze, dekodira ulazni korpus i radi
Tvrdnje zlatnog standarda bajt po bajt.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompilira samo sloj formata TU i ne zahtijeva io_uring / S3 / konstrukciju cijele biblioteke.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Izvršite `git diff --exit-code fixtures/cpp` nakon što CI regenerira `fixtures/cpp` za provjeru pomaka.

## osnovna linija

`COMPAT_BASELINE` Zaključava eloqstore predanost i razine kompatibilnosti. Nadograđena osnovna linija mora se ponovno skenirati i potvrditi.