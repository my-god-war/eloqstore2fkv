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

## Resultados actuales

Línea base del esquema 2 para 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, página de 4 KiB,
`data_append_mode=false`, `direct_io=true`. Carga de trabajo fija:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proyecto | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Poner rendimiento | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Obtener rendimiento | 5,526 QPS | 7,293 QPS | 1.32x |
| Poner ancho de banda lógico | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Obtener ancho de banda lógico | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS pico | 175.6 MiB | 145.5 MiB | 0.83x |
| Uso de almacenamiento | 269.0 MiB | 101.4 MiB | 0.38x |
| Almacenamiento/datos nominales | 2.20x | 0.83x | 0.38x |

Escribir 1 millón de entradas (10.000 lotes) corresponde a
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, es decir, los datos se sincronizan una vez por lote.
Métricas adicionales de Rust: escaneo `141,166 kv/s`, escaneo completo `108,274 kv/s`, amplificación de escritura `2.57x`
(`proc_io_write` escrituras lógicas de 329 MB/128 MB), `4.86` escribe llamadas al sistema por lote.
La relación de almacenamiento se calcula en base a 1M × 128B = 128 MB de datos nominales; C++ no activa GC/compact esta vez,
Por eso contiene más páginas de historia. No es el mismo número de calibre que "amplificación del espacio de datos de supervivencia".
La latencia de cola P99.9 de Rust es aún peor que la de C++ (ponga 223,3 ms frente a 86,0 ms, obtenga 355,5 ms frente a
48,2 ms), la línea de base única fluctúa mucho; consulte el informe de rendimiento interno para obtener más detalles.

**Nota**: `write-read` de C++ significa que el hilo de escritura y el hilo de lectura se ejecutan al mismo tiempo, y Rust actualmente
"Completar escritura, reabrir y leer nuevamente", no hay competencia de escritura en la etapa de lectura. Por lo tanto, el multiplicador de `write-read` no puede tratarse como
Conclusión pura lectura/pura escritura; comparación estricta debería ver `scripts/perf_local.sh --symmetric`
Se generan dos pares de resultados: solo escritura y solo lectura. Los números de corredor único compartido solo se usan para la regresión
Las tendencias y los lanzamientos se juzgan utilizando la mediana de `--runs`.