# Ověřovací proces

## Testování kompatibility s korozí

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` spoléhá na formát bedny `../fkv` prostřednictvím relativních cest, dekóduje vstupní korpus a dělá
Tvrzení zlatého standardu bajt po bajtu.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle pouze kompiluje formátovou vrstvu TU a nevyžaduje io_uring / S3 / celou konstrukci knihovny.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Proveďte `git diff --exit-code fixtures/cpp` po regeneraci CI `fixtures/cpp`, abyste zkontrolovali posun.

## základní linie

`COMPAT_BASELINE` Uzamyká úroveň odevzdání a kompatibility eloqstore. Upgradovaná základní linie musí být znovu naskenována a znovu ověřena.