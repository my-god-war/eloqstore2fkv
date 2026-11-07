# Proceso de verificación

## Pruebas de compatibilidad con óxido

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` se basa en la caja de formato de `../fkv` a través de rutas relativas, decodifica el corpus de entrada y no
Afirmaciones estándar de oro byte por byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle solo compila la capa de formato TU y no requiere la construcción de io_uring / S3 / biblioteca completa.

## cuerpo

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Ejecute `git diff --exit-code fixtures/cpp` después de que CI regenere `fixtures/cpp` para comprobar si hay desviación.

## base

`COMPAT_BASELINE` Bloquea los niveles de compatibilidad y confirmación de eloqstore. La línea de base actualizada debe volverse a escanear y verificar.