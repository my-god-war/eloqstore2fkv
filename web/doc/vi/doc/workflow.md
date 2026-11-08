# Quá trình xác minh

## Kiểm tra khả năng tương thích rỉ sét

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` dựa vào khung định dạng của `../fkv` thông qua các đường dẫn tương đối, giải mã kho văn bản đầu vào và thực hiện
Xác nhận tiêu chuẩn vàng theo từng byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle chỉ biên dịch lớp định dạng TU và không yêu cầu xây dựng io_uring/S3/toàn bộ thư viện.

## tử thể

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Thực thi `git diff --exit-code fixtures/cpp` sau khi CI tạo lại `fixtures/cpp` để kiểm tra độ trôi.

## đường cơ sở

`COMPAT_BASELINE` Khóa mức độ tương thích và cam kết của eloqstore. Đường cơ sở được nâng cấp phải được quét lại và xác minh lại.