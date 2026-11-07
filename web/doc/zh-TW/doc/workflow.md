# 驗證流程

## Rust 兼容測試

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` 透過相對路徑依賴 `../fkv` 的 format crate，解碼入庫語料並做
逐字節金標斷言。

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

oracle 只編譯格式層 TU，不需要 io_uring / S3 / 整庫建置。

## 語料

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

CI 重新產生 `fixtures/cpp` 後執行 `git diff --exit-code fixtures/cpp` 檢查漂移。

## 基線

`COMPAT_BASELINE` 鎖定 eloqstore commit 與相容等級。升級基線必須重掃、重驗。