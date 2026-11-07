# Proses ddilysu

## Profi cydnawsedd rhwd

```bash
cargo nextest run -p fkv_compat
```

Mae `fkv_compat` yn dibynnu ar grât fformat `../fkv` trwy lwybrau cymharol, yn dadgodio'r corpws mewnbwn ac yn gwneud hynny
Honiadau safon aur beit-wrth-beit.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Dim ond yr haen fformat TU y mae Oracle yn ei llunio ac nid oes angen io_uring / S3 / adeiladu llyfrgell gyfan.

## corpws

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Gweithredu `git diff --exit-code fixtures/cpp` ar ôl i CI adfywio `fixtures/cpp` i wirio am ddrifft.

## gwaelodlin

`COMPAT_BASELINE` Yn cloi lefelau ymrwymiad a chytunedd eloqstore. Rhaid ailsganio'r llinell sylfaen wedi'i huwchraddio a'i hailwirio.