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