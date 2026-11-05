# Rendimiento

Descargue `perf-latest` de `fkv_bench` del repositorio fkv y ejecute la misma carga de trabajo en una máquina fija.
Y compare el rendimiento, los cuantiles de latencia y el RSS máximo con una línea base de C++.

La fuente del parámetro es consistente con fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

La línea base de C++ es ejecutada por el entorno Oracle `simple_bench` y luego se genera de acuerdo con el mismo esquema y se publica en
`perf-baseline` liberar recurso. Los resultados de la comparación se escriben en el documento del almacén y el sitio genera Markdown directamente.
No se mantiene ninguna capa de datos JSON.