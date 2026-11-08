# Verifiseringsprosess

## Rustkompatibilitetstesting

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` er avhengig av formatkassen til `../fkv` gjennom relative baner, dekoder inndatakorpuset og gjør
Byte-for-byte gullstandard påstander.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompilerer kun formatlaget TU og krever ikke io_uring / S3 / hele bibliotekkonstruksjonen.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Kjør `git diff --exit-code fixtures/cpp` etter at CI regenererer `fixtures/cpp` for å se etter drift.

## grunnlinje

`COMPAT_BASELINE` Låser eloqstore commit- og kompatibilitetsnivåer. Den oppgraderte grunnlinjen må skannes på nytt og verifiseres på nytt.