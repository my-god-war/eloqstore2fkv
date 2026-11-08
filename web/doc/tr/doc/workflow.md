# Doğrulama süreci

## Pas uyumluluk testi

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat`, göreceli yollar aracılığıyla `../fkv` format kasasına güvenir, giriş kümesinin kodunu çözer ve
Bayt bayt altın standart iddiaları.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle yalnızca TU format katmanını derler ve io_uring / S3 / tüm kitaplığın oluşturulmasını gerektirmez.

## külliyat

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Sapmayı kontrol etmek için CI `fixtures/cpp` yeniden oluşturduktan sonra `git diff --exit-code fixtures/cpp` komutunu çalıştırın.

## temel çizgi

`COMPAT_BASELINE` eloqstore taahhüt ve uyumluluk seviyelerini kilitler. Yükseltilen temel yeniden taranmalı ve yeniden doğrulanmalıdır.