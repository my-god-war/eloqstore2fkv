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

## Keputusan semasa

garis dasar skema 2 untuk 06-11-2026T04:25Z,`github-ubuntu-latest-x86_64`, halaman 4 KiB,
`data_append_mode=false`, `direct_io=true`. Beban kerja tetap:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projek | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Letakkan throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Dapatkan hasil | 5,526 QPS | 7,293 QPS | 1.32x |
| Letakkan jalur lebar logik | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Dapatkan lebar jalur logik | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Puncak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Penggunaan storan | 269.0 MiB | 101.4 MiB | 0.38x |
| Penyimpanan / data nominal | 2.20x | 0.83x | 0.38x |

Menulis 1 juta penyertaan (10,000 kelompok) sepadan dengan
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, iaitu, data disegerakkan sekali setiap kelompok.
Metrik tambahan karat: Imbas `141,166 kv/s`, Fullscan `108,274 kv/s`, Tulis Amplifikasi `2.57x`
(`proc_io_write` 329 MB / 128 MB penulisan logik), `4.86` tulis panggilan sistem setiap kelompok.
Nisbah storan dikira berdasarkan 1M × 128B = 128 MB data nominal; C++ tidak mencetuskan GC/compact kali ini,
Jadi ia mengandungi lebih banyak halaman sejarah. Ia bukan nombor berkaliber yang sama dengan "penguatan ruang data kelangsungan hidup".
Kependaman ekor P99.9 Rust masih lebih teruk daripada C++ (letakkan 223.3 ms lwn 86.0 ms, dapatkan 355.5 ms lwn
48.2 ms), garis dasar tunggal sangat turun naik, sila lihat laporan prestasi dalaman untuk mendapatkan butiran.

**Nota**: `write-read` C++ bermaksud benang penulisan dan utas bacaan berjalan pada masa yang sama dan Rust pada masa ini
"Selesaikan penulisan, buka semula, dan baca semula", tiada pertandingan menulis dalam peringkat membaca. Oleh itu pengganda `write-read` tidak boleh dianggap sebagai
Bacaan tulen / kesimpulan penulisan tulen; perbandingan ketat harus melihat `scripts/perf_local.sh --symmetric`
Dua pasangan hasil adalah output: tulis sahaja dan baca sahaja. Nombor pelari kongsi tunggal hanya digunakan untuk regresi
Trend, keluaran dinilai menggunakan median `--runs`.