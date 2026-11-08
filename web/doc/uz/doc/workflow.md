# Tasdiqlash jarayoni

## Rust moslik sinovi

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` nisbiy yo'llar orqali `../fkv` format qutisiga tayanadi, kirish korpusini dekodlaydi va bajaradi
Bayt-bayt oltin standart tasdiqlar.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle faqat TU format qatlamini kompilyatsiya qiladi va io_uring / S3 / butun kutubxona qurilishini talab qilmaydi.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Drift mavjudligini tekshirish uchun CI qayta tiklangandan keyin `git diff --exit-code fixtures/cpp`ni bajaring.

## asosiy chiziq

`COMPAT_BASELINE` Eloqstore majburiyatlari va moslik darajalarini bloklaydi. Yangilangan asosiy chiziq qayta tekshirilishi va qayta tekshirilishi kerak.