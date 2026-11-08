# Performance

I-download ang `perf-latest` sa `fkv_bench` gikan sa fkv repository ug ipadagan ang samang workload sa usa ka fixed machine.
Ug itandi ang throughput, latency quantiles, ug peak RSS sa usa ka C++ baseline.

Ang tinubdan sa parameter nahiuyon sa fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Ang C++ baseline gipadagan sa oracle environment `simple_bench` ug dayon output sumala sa samang schema ug gipatik sa
`perf-baseline` buhian asset. Ang mga resulta sa pagtandi gisulat balik sa dokumento sa bodega, ug ang site direkta nga naghatag Markdown.
Walay JSON data layer nga gipadayon.

## Mga resulta karon

schema 2 baseline para sa 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB nga panid,
`data_append_mode=false`, `direct_io=true`. Naayo nga workload:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proyekto | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Ibutang ang throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Pagkuha sa throughput | 5,526 QPS | 7,293 QPS | 1.32x |
| Ibutang ang lohikal nga bandwidth | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Pagkuha og lohikal nga bandwidth | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Kinatas-ang RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Paggamit sa pagtipig | 269.0 MiB | 101.4 MiB | 0.38x |
| Pagtipig / nominal nga datos | 2.20x | 0.83x | 0.38x |

Ang pagsulat sa 1 milyon nga entries (10,000 ka batch) katumbas sa
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, sa ato pa, ang data gi-synchronize kausa kada batch.
Rust nga dugang nga metrics: Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, Write Amplification `2.57x`
(`proc_io_write` 329 MB / 128 MB logical nga pagsulat), `4.86` isulat ang mga tawag sa sistema kada batch.
Ang storage ratio gikalkulo base sa 1M × 128B = 128 MB nominal data; Ang C++ dili mag-trigger sa GC/compact karong panahona,
Mao nga kini adunay daghang mga panid sa kasaysayan. Dili kini parehas nga numero sa kalibre sa "pagpadako sa wanang sa datos sa survival".
Ang P99.9 nga tail latency sa Rust mas grabe pa sa C++ (ibutang ang 223.3 ms vs 86.0 ms, makakuha og 355.5 ms vs.
48.2 ms), ang usa ka baseline nag-usab-usab pag-ayo, palihug tan-awa ang internal nga perf report alang sa mga detalye.

**Note**: Ang `write-read` sa C++ nagpasabot nga ang writing thread ug ang reading thread dungan nga nagdagan, ug ang Rust sa pagkakaron.
"Kompleto nga pagsulat, ablihi pag-usab, ug basaha pag-usab", walay kompetisyon sa pagsulat sa yugto sa pagbasa. Busa ang multiplier sa `write-read` dili isipon nga
Putli nga pagbasa / puro nga pagsulat konklusyon; higpit nga pagtandi kinahanglan nga makita `scripts/perf_local.sh --symmetric`
Duha ka parisan sa mga resulta ang output: write-only ug read-only. Ang single shared runner nga mga numero kay gigamit lang para sa regression
Trends, releases gihukman gamit ang median sa `--runs`.