# eloqstore2fkv

Gudang verifikasi migrasi dan kompatibilitas dari eloqstore ke fkv.

Repositori fkv utama tetap murni Rust dan dibangun secara independen; repositori ini mempertahankan oracle implementasi referensi C++, korpus byte dua arah,
Migrasikan buku besar, catatan audit historis, dan verifikasi CI.

## isi

- C++ Oracle: tautkan lapisan format eloqstore dan hasilkan halaman/indeks/overflow/manifest/
  kompres korpus.
- Korpus dua arah: C++ write → Rust read, Rust write → C++ read.
- `fkv_compat`: Uji kompatibilitas sisi karat, mengandalkan format peti gudang fkv.
- `fkv_ledger`: Pindai eloqstore, catat buku besar migrasi, buat dan verifikasi korpus.
- Perbandingan kinerja: Bandingkan garis dasar fkv dan C++ pada beban kerja yang sama.

## Pintu masuk

- [Proses verifikasi](workflow.md)
- [C++ oracle](oracle.md)
- [Migrasikan buku besar](ledger.md)
- [pertunjukan](performance.md)
- [Tinjau catatan](issues.md)