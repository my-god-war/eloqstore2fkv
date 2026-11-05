# Процес перевірки

## Перевірка сумісності з іржею

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` покладається на ящик формату `../fkv` через відносні шляхи, декодує вхідний корпус і виконує
Байт за байтом твердження золотого стандарту.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle компілює лише рівень формату TU і не вимагає io_uring / S3 / побудови всієї бібліотеки.

## корпус

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Виконайте `git diff --exit-code fixtures/cpp` після того, як CI відновить `fixtures/cpp`, щоб перевірити дрейф.

## базова лінія

`COMPAT_BASELINE` Блокує фіксацію та рівні сумісності eloqstore. Оновлену базову лінію необхідно повторно відсканувати та перевірити.