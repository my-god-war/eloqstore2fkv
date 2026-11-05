# Proses pengesahan

## Ujian keserasian karat

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` bergantung pada peti format `../fkv` melalui laluan relatif, menyahkod korpus input dan melakukan
Pernyataan standard emas bait demi bait.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle hanya menyusun lapisan format TU dan tidak memerlukan io_uring / S3 / keseluruhan pembinaan perpustakaan.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Laksanakan `git diff --exit-code fixtures/cpp` selepas CI menjana semula `fixtures/cpp` untuk menyemak drift.

## garis dasar

`COMPAT_BASELINE` Mengunci tahap komit dan keserasian eloqstore. Garis dasar yang dinaik taraf mesti diimbas semula dan disahkan semula.