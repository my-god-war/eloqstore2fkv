# Vahvistusprosessi

## Ruosteen yhteensopivuuden testaus

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` luottaa muodon `../fkv` muotoiseen laatikkoon suhteellisten polkujen kautta, purkaa syöttökorpuksen ja tekee
Tavu-tavulta kultastandardin väitteet.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kääntää vain muotokerroksen TU eikä vaadi io_uring / S3 / koko kirjaston rakentamista.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Suorita `git diff --exit-code fixtures/cpp` sen jälkeen, kun CI on uusiutunut `fixtures/cpp` tarkistaaksesi ajautumisen.

## perusviiva

`COMPAT_BASELINE` Lukitsee eloqstoren sitoumus- ja yhteensopivuustasot. Päivitetty lähtötaso on tarkistettava ja vahvistettava uudelleen.