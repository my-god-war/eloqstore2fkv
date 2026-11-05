# تصديق جي عمل

## مورچا مطابقت جي جاچ

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` لاڳاپا رستا ذريعي `../fkv` جي فارميٽ ڪرٽ تي ڀاڙي ٿو، ان پٽ ڪارپس کي ڊيڪوڊ ڪري ٿو ۽ ڪندو آهي
بائيٽ بائيٽ سون جي معياري دعويٰ.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle صرف فارميٽ پرت TU کي گڏ ڪري ٿو ۽ io_uring / S3 / پوري لائبريري تعمير جي ضرورت ناهي.

## لاش

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

عمل ڪريو `git diff --exit-code fixtures/cpp` CI جي ٻيهر پيدا ٿيڻ کان پوءِ `fixtures/cpp` drift چيڪ ڪرڻ لاءِ.

## بيس لائين

`COMPAT_BASELINE` لاڪ ڪيو eloqstore عزم ۽ مطابقت جي سطح. اپڊيٽ ٿيل بيس لائين کي ٻيهر اسڪين ڪيو وڃي ۽ ٻيهر تصديق ڪيو وڃي.