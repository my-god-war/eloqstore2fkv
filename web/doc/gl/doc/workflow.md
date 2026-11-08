# Proceso de verificación

## Probas de compatibilidade de ferruxe

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` confía na caixa de formato de `../fkv` a través de camiños relativos, decodifica o corpus de entrada e fai
Afirmacións do estándar de ouro byte a byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle só compila a capa de formato TU e non require io_uring / S3 / construción de biblioteca enteira.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Execute `git diff --exit-code fixtures/cpp` despois de que CI rexenere `fixtures/cpp` para comprobar a deriva.

## liña de base

`COMPAT_BASELINE` Bloquea os niveis de compromiso e compatibilidade de eloqstore. A liña base actualizada debe volverse escanear e verificar de novo.