# Performanca

Shkarkoni `perf-latest` nga `fkv_bench` nga depoja e fkv dhe ekzekutoni të njëjtën ngarkesë pune në një makinë fikse.
Dhe krahasoni xhiros, sasitë e vonesës dhe kulmin e RSS me një bazë C++.

Burimi i parametrit është në përputhje me fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Vija bazë C++ drejtohet nga mjedisi i orakullit `simple_bench` dhe më pas del sipas të njëjtës skemë dhe publikohet në
`perf-baseline` lirim aktiv. Rezultatet e krahasimit shkruhen përsëri në dokumentin e magazinës dhe faqja jep drejtpërdrejt Markdown.
Asnjë shtresë e të dhënave JSON nuk ruhet.

## Rezultatet aktuale

Skema 2 bazë për 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, faqe 4 KiB,
`data_append_mode=false`, `direct_io=true`. Ngarkesa fikse e punës:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projekti | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Vendos xhiros | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Merrni xhiron | 5,526 QPS | 7,293 QPS | 1.32x |
| Vendosni gjerësinë e brezit logjik | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Merrni gjerësinë e brezit logjik | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Kulmi i RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Përdorimi i hapësirës ruajtëse | 269.0 MiB | 101.4 MiB | 0.38x |
| Ruajtja / të dhënat nominale | 2.20x | 0.83x | 0.38x |

Shkrimi i 1 milion hyrjeve (10,000 tufa) korrespondon me
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, domethënë, të dhënat sinkronizohen një herë për grup.
Metrikë shtesë të ndryshkut: Skano `141,166 kv/s`, skanim të plotë `108,274 kv/s`, Përforcim të shkrimit `2.57x`
(`proc_io_write` 329 MB / 128 MB shkrime logjike), `4.86` shkruani thirrjet e sistemit për grup.
Raporti i ruajtjes llogaritet në bazë të të dhënave nominale 1M × 128B = 128 MB; C++ nuk aktivizon GC/compact këtë herë,
Pra, përmban më shumë faqe historie. Nuk është i njëjti numër i kalibrit si "përforcimi i hapësirës së të dhënave të mbijetesës".
Vonesa e bishtit P99.9 e Rust është akoma më e keqe se C++ (vendos 223,3 ms kundrejt 86,0 ms, merr 355,5 ms kundrejt
48.2 ms), vija bazë e vetme luhatet shumë, ju lutemi shikoni raportin e brendshëm të perf për detaje.

**Shënim**: `write-read` e C++ do të thotë që filli i shkrimit dhe filli i leximit funksionojnë në të njëjtën kohë, dhe Rust aktualisht
“Përfundo shkrimin, rihap dhe lexo sërish”, nuk ka konkurs shkrimi në fazën e leximit. Prandaj shumëzuesi i `write-read` nuk mund të trajtohet si
Lexim i pastër / përfundim i pastër shkrimi; Krahasimi i rreptë duhet të shihet `scripts/perf_local.sh --symmetric`
Janë dalë dy palë rezultate: vetëm për shkrim dhe vetëm për lexim. Numrat e vetëm të përbashkët të vrapuesit përdoren vetëm për regresion
Tendencat, publikimet vlerësohen duke përdorur mesataren e `--runs`.