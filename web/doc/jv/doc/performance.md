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

## asil saiki

garis dasar skema 2 kanggo 06-11-2026T04:25Z,`github-ubuntu-latest-x86_64`, kaca 4 KiB,
`data_append_mode=false`, `direct_io=true`. Beban kerja tetep:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proyek | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Sijine throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Entuk throughput | 5,526 QPS | 7,293 QPS | 1.32x |
| Pasang bandwidth logis | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Entuk bandwidth logis | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Puncak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Panggunaan panyimpenan | 269.0 MiB | 101.4 MiB | 0.38x |
| Panyimpenan / data nominal | 2.20x | 0.83x | 0.38x |

Nulis 1 yuta entri (10.000 batch) cocog karo
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, yaiku, data disinkronake sapisan saben batch.
Metrik tambahan karat: Pindai `141,166 kv/s`, Fullscan `108,274 kv/s`, Tulis Amplifikasi `2.57x`
(`proc_io_write` 329 MB / 128 MB tulisan logis), `4.86` nulis panggilan sistem saben batch.
Rasio panyimpenan diitung adhedhasar data nominal 1M × 128B = 128 MB; C++ ora micu GC/compact wektu iki,
Dadi ngemot luwih akeh kaca sejarah. Iku ora nomer kaliber padha "kaslametané data spasi amplifikasi".
Latensi buntut P99.9 Rust isih luwih elek tinimbang C ++ (dilebokake 223.3 ms vs 86.0 ms, entuk 355.5 ms vs.
48,2 ms), garis dasar tunggal fluktuasi banget, deleng laporan perf internal kanggo rincian.

**Cathetan**: `write-read` C++ tegese thread nulis lan thread maca mlaku bebarengan, lan saiki Rust
“Tulis jangkep, buka maneh, lan waca maneh”, ora ana lomba nulis ing tataran maca. Mulane multiplier saka `write-read` ora bisa dianggep minangka
Wacan murni / kesimpulan tulisan murni; mbandhingake ketat kudu ndeleng `scripts/perf_local.sh --symmetric`
Rong pasangan asil minangka output: mung nulis lan mung diwaca. Nomer runner sing dienggo bareng mung digunakake kanggo regresi
Tren, rilis dinilai nggunakake median `--runs`.