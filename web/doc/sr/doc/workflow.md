# Процес верификације

## Тестирање компатибилности са рђом

```bash
cargo nextest run -p fkv_compat
```

<б ид="1"> се ослања на формат сандука <б ид="2"> кроз релативне путање, декодира улазни корпус и
Бајт по бајт златне стандардне тврдње.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Орацле само компајлира слој формата ТУ и не захтева ио_уринг / С3 / целокупну конструкцију библиотеке.

## корпус

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Извршите <б ид="2"> након што ЦИ регенерише <б ид="1"> да бисте проверили да ли постоји одступање.

## основна линија

<б ид="1"> Закључава елоксторе нивое урезивања и компатибилности. Надограђена основна линија мора бити поново скенирана и поново верификована.