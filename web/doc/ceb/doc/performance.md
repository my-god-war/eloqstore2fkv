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