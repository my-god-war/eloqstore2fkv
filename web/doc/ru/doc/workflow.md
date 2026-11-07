# Процесс проверки

## Тестирование совместимости с ржавчиной

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` использует контейнер формата `../fkv` через относительные пути, декодирует входной корпус и выполняет
Побайтовые утверждения золотого стандарта.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle компилирует только уровень формата TU и не требует построения io_uring/S3/всей библиотеки.

## корпус

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Выполните `git diff --exit-code fixtures/cpp` после того, как CI повторно создаст `fixtures/cpp`, чтобы проверить наличие отклонения.

## базовый уровень

`COMPAT_BASELINE` Блокирует уровни фиксации и совместимости eloqstore. Обновленный базовый уровень необходимо повторно отсканировать и проверить.