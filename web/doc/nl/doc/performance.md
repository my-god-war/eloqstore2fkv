# Prestaties

Download `perf-latest` van `fkv_bench` uit de fkv-repository en voer dezelfde werklast uit op een vaste machine.
En vergelijk de doorvoer, latentie-kwantielen en piek-RSS met een C++-basislijn.

De parameterbron komt overeen met fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

De C++-basislijn wordt uitgevoerd door de Oracle-omgeving `simple_bench` en vervolgens uitgevoerd volgens hetzelfde schema en gepubliceerd naar
`perf-baseline` item vrijgeven. De vergelijkingsresultaten worden teruggeschreven naar het magazijndocument en de site geeft Markdown direct weer.
Er wordt geen JSON-gegevenslaag onderhouden.