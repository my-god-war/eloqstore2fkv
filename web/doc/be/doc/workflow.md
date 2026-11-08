# Працэс праверкі

## Тэст на сумяшчальнасць з Rust

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` абапіраецца на скрыню фармату `../fkv` праз адносныя шляхі, дэкадуе ўваходны корпус і робіць
Байт за байтам зацвярджэння залатога стандарту.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle кампілюе толькі ўзровень фармату TU і не патрабуе io_uring / S3 / канструкцыі ўсёй бібліятэкі.

## корпус

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Выканайце `git diff --exit-code fixtures/cpp` пасля рэгенерацыі CI `fixtures/cpp`, каб праверыць дрэйф.

## базавая лінія

`COMPAT_BASELINE` Блакаванне ўзроўняў фіксацыі і сумяшчальнасці eloqstore. Абноўлены базавы ўзровень неабходна паўторна адсканаваць і праверыць.