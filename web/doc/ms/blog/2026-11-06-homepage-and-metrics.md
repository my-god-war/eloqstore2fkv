---
date: 2026-11-06
summary: Perbandingan prestasi halaman utama, pengembangan penunjuk penilaian, Halaman GitHub dan blog berbilang bahasa dalam talian
---

# 2026-11-06: Jadual prestasi halaman utama, pengembangan penunjuk dan Halaman

Kemas kini hari ini memfokuskan pada ulasan prestasi dan pelancaran tapak.

## Perbandingan prestasi halaman utama

- Halaman utama ditukar kepada keutamaan prestasi, dan jadual perbandingan C++/Rust dipaparkan terus pada skrin pertama.
- Tiga set penunjuk baharu ditambah: lebar jalur logik, kelewatan ekor P99.9 dan penguatan spatial.
- Menambah butiran berjalan Karat: Imbas, Imbasan Penuh, penguatan tulis, bilangan panggilan sistem tulis dan penyegerakan data setiap kelompok.
- Tandai dengan jelas `write-read` sebagai berkaliber campuran dan pautan ke dokumentasi prestasi.

Garis asas mesin yang sama semasa:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Puncak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Penggunaan storan | 269.0 MiB | 101.4 MiB | 0.38x |

## Penunjuk penilaian

Merujuk kepada output biasa RocksDB, LevelDB, YCSB, WiredTiger dan penanda aras lain, saya menambah:

- Logik membaca dan menulis lebar jalur MiB/s;
- P99.9 kelewatan ekor;
- Penguatan ruang, penguatan tulisan;
- Bilangan panggilan sistem tulis dan penyegerakan data dalam setiap kelompok;
- Imbasan / Fullscan throughput;
- CPU, kerosakan halaman, suis konteks dan proses IO terus dikumpulkan oleh sidecar sumber.

Bersara pada masa yang sama`gxhash_4096B_gib_s`. Ia ialah tanda aras cincang blok tunggal 4 KiB yang tidak berada pada laluan enjin KV dan tidak mempunyai rakan sejawat C++.

## Ketepatan dan pembetulan kejuruteraan

- Betulkan ruang kekunci `--rng cpp` baca/imbas: pengesanan baca tidak lagi didarab dengan `key_interval` untuk mengelakkan 75% bacaan jatuh ke dalam kekunci tidak bertulis.
- Percubaan semula siap sedia membetulkan yang disebabkan oleh macOS `openrsync` salah menilai manifes dengan kandungan yang sama.
- Memperbaiki isu get perkakasan yang mana cakera maya QEMU telah disalah anggap sebagai SSD tempatan.

## Tapak dan Penerbitan

- Halaman GitHub didayakan: <https://my-god-war.github.io/eloqstore2fkv/>
- Penghalaan tapak menyokong `/eloqstore2fkv/` subpath.
- Dokumen dibaca daripada `raw.githubusercontent.com` sebaliknya, dan 404 yang disebabkan oleh kehilangan `web/` dalam laluan jsDelivr dibetulkan.
- Navigasi blog dan `bun run tran` terjemahan berbilang bahasa disepadukan; artikel ini diterjemahkan secara automatik daripada sumber Cina ke dalam bahasa lain.

Log perubahan seterusnya akan terus mendokumenkan perubahan keserasian, prestasi dan rantaian alat.