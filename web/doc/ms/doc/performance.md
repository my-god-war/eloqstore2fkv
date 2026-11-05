# Prestasi

Muat turun `perf-latest` daripada `fkv_bench` daripada repositori fkv dan jalankan beban kerja yang sama pada mesin tetap.
Dan bandingkan daya pemprosesan, kuantiti kependaman dan RSS puncak kepada garis dasar C++.

Sumber parameter adalah konsisten dengan fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Garis dasar C++ dijalankan oleh persekitaran oracle `simple_bench` dan kemudian keluarkan mengikut skema yang sama dan diterbitkan ke
`perf-baseline` keluarkan aset. Hasil perbandingan ditulis kembali ke dokumen gudang, dan tapak tersebut secara langsung memberikan Markdown.
Tiada lapisan data JSON dikekalkan.