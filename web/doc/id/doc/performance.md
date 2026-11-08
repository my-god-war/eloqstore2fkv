# Performa

Unduh `perf-latest` dari `fkv_bench` dari repositori fkv dan jalankan beban kerja yang sama pada mesin tetap.
Dan bandingkan throughput, kuantil latensi, dan RSS puncak dengan garis dasar C++.

Sumber parameter konsisten dengan fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Garis dasar C++ dijalankan oleh lingkungan Oracle `simple_bench` dan kemudian dikeluarkan sesuai dengan skema yang sama dan dipublikasikan ke
`perf-baseline` melepaskan aset. Hasil perbandingan ditulis kembali ke dokumen gudang, dan situs langsung merender Penurunan Harga.
Tidak ada lapisan data JSON yang dipertahankan.

## Hasil saat ini

garis dasar skema 2 untuk 06-11-2026T04:25Z,`github-ubuntu-latest-x86_64`, halaman 4 KiB,
`data_append_mode=false`, `direct_io=true`. Beban kerja tetap:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proyek | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Masukan throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Dapatkan hasil | 5,526 QPS | 7,293 QPS | 1.32x |
| Masukkan bandwidth logis | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Dapatkan bandwidth logis | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Puncak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Penggunaan penyimpanan | 269.0 MiB | 101.4 MiB | 0.38x |
| Penyimpanan/data nominal | 2.20x | 0.83x | 0.38x |

Menulis 1 juta entri (10.000 batch) sesuai dengan
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, yaitu data disinkronkan satu kali per batch.
Metrik tambahan karat: Pindai `141,166 kv/s`, Pemindaian Penuh `108,274 kv/s`, Amplifikasi Tulis `2.57x`
(`proc_io_write` 329 MB / 128 MB penulisan logis), `4.86` menulis panggilan sistem per batch.
Rasio penyimpanan dihitung berdasarkan data nominal 1M × 128B = 128 MB; C++ tidak memicu GC/compact kali ini,
Jadi itu berisi lebih banyak halaman sejarah. Ini bukan angka kaliber yang sama dengan "amplifikasi ruang data kelangsungan hidup".
Latensi ekor P99.9 Rust masih lebih buruk daripada C++ (letakkan 223,3 ms vs 86,0 ms, dapatkan 355,5 ms vs
48,2 ms), garis dasar tunggal sangat berfluktuasi, silakan lihat laporan kinerja internal untuk mengetahui detailnya.

**Catatan**: `write-read` C++ berarti thread penulisan dan thread pembacaan berjalan pada waktu yang sama, dan Rust saat ini
“Selesaikan menulis, buka kembali, dan baca lagi”, tidak ada lomba menulis pada tahap membaca. Oleh karena itu pengali `write-read` tidak dapat dianggap sebagai
Kesimpulan bacaan murni/tulisan murni; perbandingan yang ketat akan terlihat `scripts/perf_local.sh --symmetric`
Dua pasang hasil dikeluarkan: hanya tulis dan hanya baca. Nomor pelari tunggal yang dibagikan hanya digunakan untuk regresi
Tren, rilis dinilai menggunakan median `--runs`.