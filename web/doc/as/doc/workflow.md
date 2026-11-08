# পৰীক্ষণ প্ৰক্ৰিয়া

## মৰিছা সামঞ্জস্যতা পৰীক্ষা

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` আপেক্ষিক পথসমূহৰ যোগেদি `../fkv` ৰ বিন্যাস ক্ৰেটৰ ওপৰত নিৰ্ভৰ কৰে, ইনপুট কৰ্পাছ ডিকোড কৰে আৰু কৰে
বাইট-বাই-বাইট সোণৰ প্ৰামাণিক দাবীসমূহ।

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle এ কেৱল বিন্যাস স্তৰ TU কম্পাইল কৰে আৰু io_uring / S3 / সম্পূৰ্ণ লাইব্ৰেৰী নিৰ্মাণৰ প্ৰয়োজন নাই।

## কৰ্পাছ

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

CI এ পুনৰুত্পাদন কৰাৰ পিছত `git diff --exit-code fixtures/cpp` এক্সিকিউট কৰক `fixtures/cpp` ড্ৰিফ্টৰ বাবে পৰীক্ষা কৰিবলে।

## ভিত্তি ৰেখা

`COMPAT_BASELINE` eloqstore কমিট আৰু সুসংগততা স্তৰসমূহ লক কৰে। উন্নয়ন কৰা ভিত্তিৰেখা পুনৰ স্কেন আৰু পুনৰ পৰীক্ষা কৰিব লাগিব।