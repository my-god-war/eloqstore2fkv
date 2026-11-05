# Kinerja

Ngundhuh `perf-latest` saka `fkv_bench` saka gudang fkv lan mbukak beban kerja sing padha ing mesin tetep.
Lan mbandhingake throughput, jumlah latensi, lan RSS puncak menyang garis dasar C++.

Sumber parameter konsisten karo fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ baseline dilakokno dening lingkungan oracle `simple_bench` lan banjur output miturut skema sing padha lan diterbitake menyang
`perf-baseline` rilis aset. Asil comparison ditulis bali menyang document warehouse, lan situs langsung render Markdown.
Ora ana lapisan data JSON sing dijaga.