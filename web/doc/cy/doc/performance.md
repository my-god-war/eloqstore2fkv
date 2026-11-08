# Perfformiad

Lawrlwythwch `perf-latest` o `fkv_bench` o'r ystorfa fkv a rhedeg yr un llwyth gwaith ar beiriant sefydlog.
A chymharwch trwygyrch, meintiau cuddni, a RSS brig i linell sylfaen C++.

Mae ffynhonnell y paramedr yn gyson â fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Mae'r llinell sylfaen C++ yn cael ei rhedeg gan yr amgylchedd oracle `simple_bench` ac yna'n allbwn yn ôl yr un sgema a'i chyhoeddi i
`perf-baseline` rhyddhau ased. Mae canlyniadau'r gymhariaeth yn cael eu hysgrifennu yn ôl i'r ddogfen warws, ac mae'r wefan yn gwneud Markdown yn uniongyrchol.
Nid oes haen ddata JSON yn cael ei chynnal.

## Canlyniadau cyfredol

llinell sylfaen sgema 2 ar gyfer 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, tudalen 4 KiB,
`data_append_mode=false`, `direct_io=true`. Llwyth gwaith sefydlog:
`rows/max_key=1000000`, `batch_size=100`, `write_batchs=10000`,
`read_per_part=4`, `partitions=1`, `kv_size=128`, `rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| prosiect | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Rhowch trwybwn | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Cael trwybwn | 5,526 QPS | 7,293 QPS | 1.32x |
| Rhowch lled band rhesymegol | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Cael lled band rhesymegol | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS brig | 175.6 MiB | 145.5 MiB | 0.83x |
| Defnydd storio | 269.0 MiB | 101.4 MiB | 0.38x |
| Data storio / enwol | 2.20x | 0.83x | 0.38x |

Mae ysgrifennu 1 miliwn o gofnodion (10,000 sypiau) yn cyfateb i
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, hynny yw, mae data'n cael eu cysoni unwaith fesul swp.
Rhwdiwch fetrigau ychwanegol: Sgan `141,166 kv/s`, Fullscan `108,274 kv/s`, Ysgrifennwch Amplification `2.57x`
(`proc_io_write` 329 MB / 128 MB yn ysgrifennu rhesymegol), `4.86` ysgrifennu galwadau system fesul swp.
Cyfrifir y gymhareb storio yn seiliedig ar ddata enwol 1M × 128B = 128 MB; Nid yw C++ yn sbarduno GC/compact y tro hwn,
Felly mae'n cynnwys mwy o dudalennau o hanes. Nid yw o'r un rhif caliber â "chwyddo gofod data goroesi".
Mae hwyrni cynffon P99.9 Rust yn dal yn waeth na C++ (rhowch 223.3 ms yn erbyn 86.0 ms, mynnwch 355.5 ms vs
48.2 ms), mae'r llinell sylfaen sengl yn amrywio'n fawr, gweler yr adroddiad perf mewnol am fanylion.

**Sylwer**: Mae `write-read` C++ yn golygu bod yr edefyn ysgrifennu a'r edefyn darllen yn rhedeg ar yr un pryd, a Rust ar hyn o bryd
"Ysgrifennu cyflawn, ailagor, a darllen eto", nid oes cystadleuaeth ysgrifennu yn y cyfnod darllen. Felly ni ellir trin y lluosydd `write-read` fel
Casgliad darllen pur / ysgrifennu pur; dylai cymhariaeth fanwl weld `scripts/perf_local.sh --symmetric`
Mae dau bâr o ganlyniadau yn allbwn: ysgrifennu yn unig a darllen yn unig. Dim ond ar gyfer atchweliad y defnyddir rhifau rhedwr a rennir sengl
Tueddiadau, caiff datganiadau eu barnu gan ddefnyddio canolrif `--runs`.