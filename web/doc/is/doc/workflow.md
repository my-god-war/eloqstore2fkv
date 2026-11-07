# Staðfestingarferli

## Ryðsamhæfisprófun

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` byggir á sniði rimlakassi `../fkv` í gegnum afstæðar slóðir, afkóðar inntaksheildina og gerir
Bæti fyrir bæti fullyrðingar um gullstaðal.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle setur aðeins saman sniðlagið TU og krefst ekki io_uring / S3 / allt bókasafnsbyggingar.

## málheild

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Keyrðu `git diff --exit-code fixtures/cpp` eftir að CI endurskapar `fixtures/cpp` til að athuga hvort reki sé.

## grunnlínu

`COMPAT_BASELINE` Læsir eloqstore skuldbindingar- og samhæfnistigum. Uppfærða grunnlínan verður að endurskoða og staðfesta aftur.