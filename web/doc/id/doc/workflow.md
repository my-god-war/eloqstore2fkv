# Proses verifikasi

## Pengujian kompatibilitas karat

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` bergantung pada format peti `../fkv` melalui jalur relatif, menerjemahkan kode korpus masukan dan melakukan
Penegasan standar emas byte demi byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle hanya mengkompilasi lapisan format TU dan tidak memerlukan konstruksi io_uring/S3/seluruh perpustakaan.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Jalankan `git diff --exit-code fixtures/cpp` setelah CI membuat ulang `fixtures/cpp` untuk memeriksa penyimpangan.

## garis dasar

`COMPAT_BASELINE` Mengunci tingkat penerapan dan kompatibilitas eloqstore. Garis dasar yang ditingkatkan harus dipindai ulang dan diverifikasi ulang.