---
date: 2026-11-06
summary: Perbandingan kinerja kaca ngarep, ekspansi indikator evaluasi, Kaca GitHub lan blog multi-basa online
---

# 2026-11-06: Tabel kinerja kaca ngarep, ekspansi indikator lan Kaca

Nganyari saiki fokus ing review kinerja lan peluncuran situs.

## Perbandingan kinerja kaca ngarep

- Homepage diganti dadi prioritas kinerja, lan tabel comparison C ++ / Rust langsung ditampilake ing layar pisanan.
- Telung set indikator anyar ditambahake: bandwidth logis, tundha buntut P99.9, lan amplifikasi spasial.
- Ditambahake Rincian Rust mlaku: Scan, Fullscan, nulis amplifikasi, nomer nulis telpon sistem lan sinkronisasi data saben kumpulan.
- Tandha kanthi jelas `write-read` minangka kaliber campuran lan pranala menyang dokumentasi kinerja.

Baseline mesin sing padha saiki:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Puncak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Panggunaan panyimpenan | 269.0 MiB | 101.4 MiB | 0.38x |

## Indikator evaluasi

Ngrujuk marang output umum RocksDB, LevelDB, YCSB, WiredTiger lan pathokan liyane, aku nambahake:

- Logis maca lan nulis bandwidth MiB/s;
- P99.9 tundha buntut;
- amplifikasi spasi, amplifikasi nulis;
- Jumlah telpon sistem nulis lan sinkronisasi data ing saben batch;
- Pindai / Fullscan throughput;
- CPU, kesalahan kaca, switch konteks, lan proses IO terus diklumpukake dening sidecar sumber.

Pensiun ing wektu sing padha`gxhash_4096B_gib_s`. Iku microbenchmark hash blok tunggal 4 KiB sing ora ana ing jalur mesin KV lan ora duwe pasangan C ++.

## Correctness lan mbenakake engineering

- Ndandani spasi tombol `--rng cpp` waca/pindai: deteksi waca ora dikaliake maneh karo `key_interval` supaya 75% wacan ora ana ing tombol sing ora ditulis.
- Coba maneh siyaga sing didandani amarga kesalahan MacOS `openrsync` kanggo manifes kanthi konten sing padha.
- Ndandani masalah gerbang hardware ing ngendi disk virtual QEMU disalahake minangka SSD lokal.

## Situs lan Publishing

- Kaca GitHub diaktifake: <https://my-god-war.github.io/eloqstore2fkv/>
- Rute situs ndhukung `/eloqstore2fkv/` subpath.
- Dokumen diwaca saka `raw.githubusercontent.com` tinimbang, lan 404 sing disebabake ilang `web/` ing dalan jsDelivr didandani.
- Navigasi blog lan `bun run tran` terjemahan multi-basa digabungake; artikel iki diterjemahaké kanthi otomatis saka sumber Cina menyang basa liyane.

Changelogs sabanjure bakal terus nyathet kompatibilitas, kinerja, lan owah-owahan toolchain.