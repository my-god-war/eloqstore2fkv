# eloqstore2fkv

Migrasi lan gudang verifikasi kompatibilitas saka eloqstore menyang fkv.

Repositori fkv utama tetep Rust murni lan dibangun kanthi mandiri; repositori iki nahan oracle implementasi referensi C++, korpus byte bidirectional,
Migrasi buku besar, cathetan audit historis lan verifikasi CI.

## isi

- Oracle C++: ngubungake lapisan format eloqstore lan ngasilake kaca/index/overflow/manifest/
  kompres corpus.
- Korpus bidirectional: C++ nulis → Rust maca, Rust nulis → C++ maca.
- `fkv_compat`: Tes kompatibilitas ing sisih karat, gumantung ing format peti gudang fkv.
- `fkv_ledger`: Pindai eloqstore, rekam buku besar migrasi, gawe lan verifikasi korpus.
- Perbandingan kinerja: Mbandhingake garis dasar fkv lan C ++ ing beban kerja sing padha.

## mlebu

- [Proses verifikasi](workflow.md)
- [C++ oracle](oracle.md)
- [Migrasi ledger](ledger.md)
- [kinerja](performance.md)
- [Review cathetan](issues.md)