# Veiktspēja

Lejupielādējiet `perf-latest` no `fkv_bench` no fkv repozitorija un palaidiet to pašu darba slodzi fiksētā datorā.
Un salīdziniet caurlaidspēju, latentuma kvantiles un maksimālo RSS ar C++ bāzes līniju.

Parametra avots atbilst fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ bāzes līniju palaiž Oracle vide `simple_bench` un pēc tam izvada saskaņā ar to pašu shēmu un publicē
`perf-baseline` atbrīvot līdzekli. Salīdzināšanas rezultāti tiek ierakstīti atpakaļ noliktavas dokumentā, un vietne tieši atveido Markdown.
JSON datu slānis netiek uzturēts.