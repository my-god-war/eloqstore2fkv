# تصدیق کا عمل

## زنگ کی مطابقت کی جانچ

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` رشتہ دار راستوں کے ذریعے `../fkv` کے فارمیٹ کریٹ پر انحصار کرتا ہے، ان پٹ کارپس کو ڈی کوڈ کرتا ہے اور کرتا ہے
بائٹ بہ بائٹ سونے کے معیاری دعوے

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

اوریکل صرف فارمیٹ پرت TU کو مرتب کرتا ہے اور اسے io_uring / S3 / پوری لائبریری کی تعمیر کی ضرورت نہیں ہے۔

## کارپس

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

CI کے دوبارہ پیدا ہونے کے بعد `git diff --exit-code fixtures/cpp` پر عمل درآمد کریں `fixtures/cpp` بڑھے کو چیک کرنے کے لیے۔

## بیس لائن

`COMPAT_BASELINE` eloqstore کمٹ اور مطابقت کی سطحوں کو لاک کرتا ہے۔ اپ گریڈ شدہ بیس لائن کو دوبارہ اسکین اور دوبارہ تصدیق کرنا ضروری ہے۔