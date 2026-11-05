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