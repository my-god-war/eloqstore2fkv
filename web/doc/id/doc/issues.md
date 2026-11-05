# Tinjau catatan

`issue/` adalah papan peninjau selama migrasi:

- `new/`: Entri untuk komentar baru.
- `todo/`: Diadopsi, menunggu pengembangan.
- `done/`: Dikembangkan dan memiliki bukti pengujian, menunggu peninjauan.
- `reviewed/`: Arsip ditinjau.
- `reject/`: Jika tidak diadopsi, sebutkan alasannya.

Lihat `issue/new.md`, `issue/run.md`, `issue/review.md` untuk prosesnya.

Tinjauan ini berfokus pada perbedaan perilaku, kompatibilitas format, kinerja, dan struktur kode antara implementasi fkv dan C++. Satu pendapat per
file untuk menghindari perpindahan massal.