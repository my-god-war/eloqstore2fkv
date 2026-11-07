---
date: 2026-11-06
summary: Perbandingan kinerja halaman beranda, perluasan indikator evaluasi, Halaman GitHub dan blog multi-bahasa online
---

# 06-11-2026: Tabel kinerja halaman beranda, perluasan indikator, dan Halaman

Pembaruan hari ini berfokus pada tinjauan kinerja dan peluncuran situs.

## Perbandingan kinerja halaman beranda

- Beranda diubah menjadi prioritas kinerja, dan tabel perbandingan C++/Rust langsung ditampilkan di layar pertama.
- Tiga set indikator baru ditambahkan: bandwidth logis, penundaan ekor P99.9, dan amplifikasi spasial.
- Menambahkan detail pengoperasian Rust: Pemindaian, Pemindaian Penuh, amplifikasi tulis, jumlah panggilan sistem tulis, dan sinkronisasi data per batch.
- Tandai secara eksplisit `write-read` sebagai kaliber campuran dan tautkan ke dokumentasi kinerja.

Baseline mesin yang sama saat ini:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Puncak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Penggunaan penyimpanan | 269.0 MiB | 101.4 MiB | 0.38x |

## Indikator evaluasi

Mengacu pada keluaran umum RocksDB, LevelDB, YCSB, WiredTiger dan tolok ukur lainnya, saya menambahkan:

- Bandwidth baca dan tulis logis MiB/s;
- penundaan ekor P99.9;
- Amplifikasi ruang, amplifikasi tulisan;
- Jumlah panggilan sistem tulis dan sinkronisasi data di setiap batch;
- Throughput Pindai / Pemindaian Penuh;
- CPU, kesalahan halaman, peralihan konteks, dan IO proses terus dikumpulkan oleh sidecar sumber daya.

Pensiun pada waktu yang sama`gxhash_4096B_gib_s`. Ini adalah microbenchmark hash blok tunggal 4 KiB yang tidak berada di jalur mesin KV dan tidak memiliki mitra C++.

## Perbaikan kebenaran dan rekayasa

- Perbaiki ruang kunci `--rng cpp` baca/pindai: deteksi baca tidak lagi dikalikan dengan `key_interval` untuk menghindari 75% pembacaan jatuh ke kunci tidak tertulis.
- Memperbaiki percobaan ulang siaga yang disebabkan oleh kesalahan penilaian macOS `openrsync` terhadap manifes dengan konten yang sama.
- Memperbaiki masalah gerbang perangkat keras yang menyebabkan disk virtual QEMU salah dinilai sebagai SSD lokal.

## Situs dan Penerbitan

- Halaman GitHub diaktifkan: <https://my-god-war.github.io/eloqstore2fkv/>
- Perutean situs mendukung subjalur `/eloqstore2fkv/`.
- Dokumen dibaca dari `raw.githubusercontent.com` sebagai gantinya, dan 404 yang disebabkan oleh hilangnya `web/` di jalur jsDelivr telah diperbaiki.
- Navigasi blog dan `bun run tran` terjemahan multi-bahasa terintegrasi; artikel ini secara otomatis diterjemahkan dari sumber berbahasa Mandarin ke bahasa lain.

Log perubahan berikutnya akan terus mendokumentasikan perubahan kompatibilitas, kinerja, dan rantai alat.