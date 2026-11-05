# Процес на проверка

## Тест за съвместимост с ръжда

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` разчита на кутията за форматиране на `../fkv` чрез относителни пътища, декодира входния корпус и прави
Байт по байт твърдения за златен стандарт.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle компилира само слоя формат TU и не изисква io_uring / S3 / цялостна конструкция на библиотеката.

## корпус

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Изпълнете `git diff --exit-code fixtures/cpp`, след като CI регенерира `fixtures/cpp`, за да проверите за дрейф.

## базова линия

`COMPAT_BASELINE` Заключва нивата на ангажираност и съвместимост на eloqstore. Надстроената базова линия трябва да бъде повторно сканирана и проверена.