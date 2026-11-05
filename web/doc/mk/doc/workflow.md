# Процес на верификација

## Тестирање на компатибилност со 'рѓа

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` се потпира на формат гајба на `../fkv` низ релативни патеки, го декодира влезниот корпус и прави
Бајт-по-бајт тврдења за златен стандард.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle само го составува слојот за формат TU и не бара io_uring / S3 / целата конструкција на библиотеката.

## корпус

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Извршете `git diff --exit-code fixtures/cpp` откако CI ќе се регенерира `fixtures/cpp` за да проверите дали има дрифт.

## основната линија

`COMPAT_BASELINE` Заклучување на eloqstore и нивоа на компатибилност. Надградената основна линија мора повторно да се скенира и да се потврди.