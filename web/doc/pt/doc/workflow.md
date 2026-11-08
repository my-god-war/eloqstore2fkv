# Processo de verificação

## Teste de compatibilidade com ferrugem

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` depende da caixa de formato de `../fkv` através de caminhos relativos, decodifica o corpus de entrada e faz
Asserções do padrão ouro byte a byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

O Oracle compila apenas a camada de formato TU e não requer io_uring/S3/construção inteira da biblioteca.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Execute `git diff --exit-code fixtures/cpp` depois que o CI regenerar `fixtures/cpp` para verificar desvios.

## linha de base

`COMPAT_BASELINE` Bloqueia os níveis de commit e compatibilidade do eloqstore. A linha de base atualizada deve ser verificada novamente e verificada novamente.