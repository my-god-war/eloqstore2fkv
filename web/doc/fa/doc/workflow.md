# فرآیند تأیید

## تست سازگاری زنگ

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` از طریق مسیرهای نسبی به جعبه قالب `../fkv` متکی است، پیکره ورودی را رمزگشایی می کند و انجام می دهد
اظهارات بایت به بایت استاندارد طلا.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

اوراکل فقط لایه قالب TU را کامپایل می کند و نیازی به ساخت io_uring / S3 / کل کتابخانه ندارد.

## بدنه

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

`git diff --exit-code fixtures/cpp` را بعد از ایجاد مجدد CI `fixtures/cpp` اجرا کنید تا drift را بررسی کنید.

## خط پایه

`COMPAT_BASELINE` سطوح تعهد و سازگاری Locks eloqstore. خط پایه ارتقا یافته باید دوباره اسکن و تأیید شود.