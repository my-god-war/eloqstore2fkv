# Postopek preverjanja

## Testiranje združljivosti z rjo

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` se opira na zaboj formata `../fkv` prek relativnih poti, dekodira vhodni korpus in
Trditve zlatega standarda bajt za bajtom.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle prevede samo plast formata TU in ne zahteva io_uring / S3 / konstrukcije celotne knjižnice.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Izvedite `git diff --exit-code fixtures/cpp`, potem ko CI regenerira `fixtures/cpp`, da preverite premik.

## izhodišče

`COMPAT_BASELINE` Zaklene ravni zavezanosti in združljivosti eloqstore. Nadgrajeno osnovno linijo je treba znova skenirati in preveriti.