# Leistung

Laden Sie `perf-latest` von `fkv_bench` aus dem fkv-Repository herunter und führen Sie denselben Workload auf einem festen Computer aus.
Und vergleichen Sie Durchsatz, Latenzquantile und Spitzen-RSS mit einer C++-Basislinie.

Die Parameterquelle stimmt mit fkv überein:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Die C++-Baseline wird von der Oracle-Umgebung `simple_bench` ausgeführt und dann nach demselben Schema ausgegeben und veröffentlicht
`perf-baseline` Asset freigeben. Die Vergleichsergebnisse werden in das Lagerdokument zurückgeschrieben und die Site rendert Markdown direkt.
Es wird keine JSON-Datenschicht gepflegt.