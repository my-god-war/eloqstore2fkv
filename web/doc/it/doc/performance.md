# Prestazioni

Scarica `perf-latest` di `fkv_bench` dal repository fkv ed esegui lo stesso carico di lavoro su un computer fisso.
E confronta il throughput, i quantili di latenza e il picco RSS con una linea di base C++.

La sorgente del parametro è coerente con fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

La linea di base C++ viene eseguita dall'ambiente Oracle `simple_bench` e quindi generata secondo lo stesso schema e pubblicata in
`perf-baseline` rilasciare risorsa. I risultati del confronto vengono riscritti nel documento di warehouse e il sito esegue direttamente il rendering di Markdown.
Non viene mantenuto alcun livello dati JSON.