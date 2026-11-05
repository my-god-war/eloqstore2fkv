# যাচাইকরণ প্রক্রিয়া

## মরিচা সামঞ্জস্য পরীক্ষা

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` আপেক্ষিক পাথের মাধ্যমে `../fkv` ফর্ম্যাট ক্রেটের উপর নির্ভর করে, ইনপুট কর্পাস ডিকোড করে এবং করে
বাইট-বাই-বাইট গোল্ড স্ট্যান্ডার্ড দাবী।

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

ওরাকল শুধুমাত্র ফরম্যাট লেয়ার TU কম্পাইল করে এবং এর জন্য io_uring/S3/ সম্পূর্ণ লাইব্রেরি নির্মাণের প্রয়োজন হয় না।

## কর্পাস

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

ড্রিফ্ট চেক করতে CI পুনরায় তৈরি করার পরে `git diff --exit-code fixtures/cpp` চালান৷

## ভিত্তিরেখা

`COMPAT_BASELINE` eloqstore কমিট এবং সামঞ্জস্যের স্তর লক করে। আপগ্রেড করা বেসলাইন পুনরায় স্ক্যান এবং পুনরায় যাচাই করা আবশ্যক।