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