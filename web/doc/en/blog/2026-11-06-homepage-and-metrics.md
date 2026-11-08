---
date: 2026-11-06
summary: Home page performance comparison, evaluation indicator expansion, GitHub Pages and multi-language blogs online
---

# 2026-11-06: Home page performance table, indicator expansion and Pages

Today's update focuses on performance reviews and site launches.

## Home page performance comparison

- The homepage is changed to performance priority, and the C++/Rust comparison table is directly displayed on the first screen.
- Three new sets of indicators are added: logical bandwidth, P99.9 tail delay, and spatial amplification.
- Added Rust running details: Scan, Fullscan, write amplification, number of write system calls and data synchronization per batch.
- Explicitly mark `write-read` as mixed caliber and link to performance documentation.

Current same-machine baseline:

| Index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Storage usage | 269.0 MiB | 101.4 MiB | 0.38x |

## Evaluation indicators

Referring to the common output of RocksDB, LevelDB, YCSB, WiredTiger and other benchmarks, I added:

- Logical read and write bandwidth MiB/s;
- P99.9 tail delay;
- Space amplification, writing amplification;
- Number of write system calls and data synchronization in each batch;
- Scan / Fullscan throughput;
- CPU, page faults, context switches, and process IO continue to be collected by the resource sidecar.

Retired at the same time`gxhash_4096B_gib_s`. It is a 4 KiB single block hash microbenchmark that is not on the KV engine path and has no C++ counterpart.

## Correctness and engineering fixes

- Fix the key space of `--rng cpp` read/scan: read detection is no longer multiplied by `key_interval` to avoid 75% of reads falling into unwritten keys.
- Fixed standby retries caused by macOS `openrsync` misjudgment of manifests with the same content.
- Fixed a hardware gate issue where QEMU virtual disks were misjudged as local SSDs.

## Sites and Publishing

- GitHub Pages enabled: <https://my-god-war.github.io/eloqstore2fkv/>
- Site routing supports `/eloqstore2fkv/` subpaths.
- The document is read from `raw.githubusercontent.com` instead, and the 404 caused by missing `web/` in the jsDelivr path is fixed.
- Blog navigation and `bun run tran` multi-language translation are integrated; this article is automatically translated from the Chinese source into other languages.

Subsequent changelogs will continue to document compatibility, performance, and toolchain changes.