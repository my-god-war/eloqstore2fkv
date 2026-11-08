# Proces verifikacije

## Testiranje kompatibilnosti sa hrđom

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` se oslanja na format sanduka `../fkv` kroz relativne staze, dekodira ulazni korpus i čini
Bajt po bajt zlatne standardne tvrdnje.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle samo kompajlira sloj formata TU i ne zahtijeva io_uring / S3 / cjelokupnu konstrukciju biblioteke.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Izvršite `git diff --exit-code fixtures/cpp` nakon što CI regenerira `fixtures/cpp` da provjerite odstupanje.

## osnovna linija

`COMPAT_BASELINE` Zaključava eloqstore urezivanje i nivoe kompatibilnosti. Nadograđena osnovna linija mora biti ponovo skenirana i ponovo potvrđena.