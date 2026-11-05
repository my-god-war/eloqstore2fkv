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