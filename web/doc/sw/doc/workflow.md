# Mchakato wa uthibitishaji

## Mtihani wa utangamano wa kutu

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` inategemea kreti ya umbizo la `../fkv` kupitia njia jamaa, inabainisha mkusanyiko wa ingizo na kufanya
Madai ya kawaida ya dhahabu ya Byte-byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle inakusanya tu safu ya umbizo TU na haihitaji io_uring / S3 / ujenzi wa maktaba nzima.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Tekeleza `git diff --exit-code fixtures/cpp` baada ya CI kutengeneza upya `fixtures/cpp` ili kuangalia kama kuna utelezi.

## msingi

`COMPAT_BASELINE` Hufunga ahadi za eloqstore na viwango vya uoanifu. Msingi ulioboreshwa lazima uchanganuliwe upya na uthibitishwe tena.