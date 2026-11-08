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

## Risultati attuali

linea di base dello schema 2 per 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, pagina da 4 KiB,
`data_append_mode=false`, `direct_io=true`. Carico di lavoro fisso:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| progetto | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Metti il ​​throughput | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Ottieni produttività | 5,526 QPS | 7,293 QPS | 1.32x |
| Metti la larghezza di banda logica | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Ottieni larghezza di banda logica | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Picco RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Utilizzo dello spazio di archiviazione | 269.0 MiB | 101.4 MiB | 0.38x |
| Archiviazione/dati nominali | 2.20x | 0.83x | 0.38x |

La scrittura di 1 milione di voci (10.000 lotti) corrisponde a
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, ovvero i dati vengono sincronizzati una volta per batch.
Parametri aggiuntivi di Rust: scansione `141,166 kv/s`, scansione completa `108,274 kv/s`, amplificazione di scrittura `2.57x`
(`proc_io_write` 329 MB / 128 MB di scritture logiche), `4.86` scrive chiamate di sistema per batch.
Il rapporto di archiviazione viene calcolato in base a 1M × 128B = 128 MB di dati nominali; C++ non attiva GC/compact questa volta,
Quindi contiene più pagine di storia. Non è dello stesso calibro dell'"amplificazione dello spazio dei dati di sopravvivenza".
La latenza della coda P99.9 di Rust è ancora peggiore di quella del C++ (metti 223,3 ms contro 86,0 ms, ottieni 355,5 ms contro
48,2 ms), la singola linea di base fluttua notevolmente, per i dettagli consultare il rapporto sulle prestazioni interno.

**Nota**: `write-read` di C++ significa che il thread di scrittura e il thread di lettura vengono eseguiti contemporaneamente e Rust attualmente
"Completa la scrittura, riapri e rileggi", non è prevista alcuna gara di scrittura nella fase di lettura. Pertanto il moltiplicatore di `write-read` non può essere trattato come
Conclusione di pura lettura/pura scrittura; il confronto rigoroso dovrebbe vedere `scripts/perf_local.sh --symmetric`
Vengono emesse due coppie di risultati: di sola scrittura e di sola lettura. I singoli numeri di corridori condivisi vengono utilizzati solo per la regressione
Tendenze e rilasci vengono valutati utilizzando la mediana di `--runs`.