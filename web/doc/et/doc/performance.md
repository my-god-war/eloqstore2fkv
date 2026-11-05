# Jõudlus

Laadige fkv hoidlast alla `perf-latest` / `fkv_bench` ja käivitage sama töökoormus fikseeritud masinas.
Ja võrrelge läbilaskevõimet, latentsuskvantiile ja tipp-RSS-i C++ baastasemega.

Parameetri allikas on kooskõlas fkv-ga:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ baasjoont käivitab Oracle'i keskkond `simple_bench` ja seejärel väljastatakse sama skeemi järgi ja avaldatakse
`perf-baseline` vabastage vara. Võrdlustulemused kirjutatakse tagasi laodokumenti ja sait renderdab otse Markdowni.
JSON-i andmekihti ei hooldata.