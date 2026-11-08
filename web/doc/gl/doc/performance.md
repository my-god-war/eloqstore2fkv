# Rendemento

Descarga `perf-latest` de `fkv_bench` desde o repositorio fkv e executa a mesma carga de traballo nunha máquina fixa.
E compara o rendemento, os cuantiles de latencia e o pico de RSS cunha liña base de C++.

A fonte do parámetro é coherente con fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

A liña base de C++ é executada polo ambiente Oracle `simple_bench` e, a continuación, sae segundo o mesmo esquema e publícase en
`perf-baseline` liberar recurso. Os resultados da comparación son escritos de novo no documento do almacén e o sitio representa directamente Markdown.
Non se mantén ningunha capa de datos JSON.

## Resultados actuais

esquema 2 de referencia para o 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, páxina de 4 KiB,
`data_append_mode=false`, `direct_io=true`. Carga de traballo fixa:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proxecto | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Poñer rendemento | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Obter rendemento | 5,526 QPS | 7,293 QPS | 1.32x |
| Poñer ancho de banda lóxico | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Obter ancho de banda lóxico | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS pico | 175.6 MiB | 145.5 MiB | 0.83x |
| Uso de almacenamento | 269.0 MiB | 101.4 MiB | 0.38x |
| Almacenamento/datos nominais | 2.20x | 0.83x | 0.38x |

Escribir 1 millón de entradas (10.000 lotes) corresponde a
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, é dicir, os datos sincronízanse unha vez por lote.
Métricas adicionais de oxidación: exploración `141,166 kv/s`, exploración completa `108,274 kv/s`, amplificación de escritura `2.57x`
(`proc_io_write` 329 MB / 128 MB de escrituras lóxicas), `4.86` escribe chamadas de sistema por lote.
A relación de almacenamento calcúlase en función de 1M × 128B = 128 MB de datos nominais; C++ non activa GC/compact esta vez,
Polo que contén máis páxinas de historia. Non é o mesmo número de calibre que "amplificación do espazo de datos de supervivencia".
A latencia de cola P99.9 de Rust aínda é peor que C++ (pon 223,3 ms fronte a 86,0 ms, obtén 355,5 ms fronte a
48,2 ms), a única liña de base varía moito, consulte o informe de rendemento interno para obter máis información.

**Nota**: o `write-read` de C++ significa que o fío de escritura e o fío de lectura se executan ao mesmo tempo, e Rust actualmente
"Completa a escritura, reabre e volve ler", non hai concurso de escritura na fase de lectura. Polo tanto, o multiplicador de `write-read` non se pode tratar como
Pura lectura / pura conclusión escrita; unha comparación estrita debería ver `scripts/perf_local.sh --symmetric`
Saen dous pares de resultados: só de escritura e só de lectura. Os números de corredor único compartido só se usan para a regresión
As tendencias e as publicacións xulganse utilizando a mediana de `--runs`.