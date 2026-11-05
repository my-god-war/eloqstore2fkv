# Egiaztatzeko prozesua

## Herdoilaren bateragarritasun-probak

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` bide erlatiboen bidez `../fkv`-ren formatu-kutxan oinarritzen da, sarrerako corpusa deskodetzen du eta
Bytez byte urre estandarraren baieztapenak.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle-k TU formatu geruza bakarrik konpilatzen du eta ez du io_uring / S3 / liburutegi osoa eraikitzeko behar.

## corpusa

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Exekutatu `git diff --exit-code fixtures/cpp` CI birsortu ondoren `fixtures/cpp` noraeza egiaztatzeko.

## oinarri-lerroa

`COMPAT_BASELINE` eloqstore konpromisoa eta bateragarritasun mailak blokeatzen ditu. Berritutako oinarri-lerroa berriro aztertu eta berriro egiaztatu behar da.