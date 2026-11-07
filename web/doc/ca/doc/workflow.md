# Procés de verificació

## Proves de compatibilitat amb l'òxid

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` es basa en la caixa de format de `../fkv` a través de camins relatius, descodifica el corpus d'entrada i fa
Afirmacions estàndard d'or byte a byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle només compila la capa de format TU i no requereix io_uring / S3 / construcció de biblioteca sencera.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Executeu `git diff --exit-code fixtures/cpp` després que CI es regeneri `fixtures/cpp` per comprovar si hi ha deriva.

## línia de base

`COMPAT_BASELINE` Bloqueja els nivells de compromís i compatibilitat d'eloqstore. La línia de base actualitzada s'ha de tornar a escanejar i verificar de nou.