---
date: 2026-11-06
summary: Comparación del rendimiento de la página de inicio, expansión del indicador de evaluación, páginas de GitHub y blogs en línea en varios idiomas
---

# 2026-11-06: Tabla de rendimiento de la página de inicio, expansión de indicadores y Páginas

La actualización de hoy se centra en revisiones de rendimiento y lanzamientos de sitios.

## Comparación del rendimiento de la página de inicio

- La página de inicio cambia a prioridad de rendimiento y la tabla de comparación C++/Rust se muestra directamente en la primera pantalla.
- Se agregan tres nuevos conjuntos de indicadores: ancho de banda lógico, retardo de cola P99.9 y amplificación espacial.
- Se agregaron detalles de ejecución de Rust: escaneo, escaneo completo, amplificación de escritura, número de llamadas al sistema de escritura y sincronización de datos por lote.
- Marque explícitamente `write-read` como calibre mixto y enlace a la documentación de rendimiento.

Línea base actual en la misma máquina:

| índice | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS pico | 175.6 MiB | 145.5 MiB | 0.83x |
| Uso de almacenamiento | 269.0 MiB | 101.4 MiB | 0.38x |

## Indicadores de evaluación

Refiriéndose al resultado común de RocksDB, LevelDB, YCSB, WiredTiger y otros puntos de referencia, agregué:

- Ancho de banda de lectura y escritura lógica MiB/s;
- P99.9 retraso de cola;
- Ampliación de espacio, amplificación de escritura;
- Número de llamadas al sistema de escritura y sincronización de datos en cada lote;
- Rendimiento de escaneo/escaneo completo;
- El sidecar de recursos continúa recopilando CPU, fallas de página, cambios de contexto y E/S de proceso.

Jubilado al mismo tiempo`gxhash_4096B_gib_s`. Es un microbenchmark hash de bloque único de 4 KiB que no está en la ruta del motor KV y no tiene contraparte en C++.

## Corrección y correcciones de ingeniería.

- Corrija el espacio clave de `--rng cpp` lectura/escaneo: la detección de lectura ya no se multiplica por `key_interval` para evitar que el 75% de las lecturas caigan en claves no escritas.
- Se corrigieron los reintentos en espera causados ​​por macOS `openrsync` juicio erróneo de manifiestos con el mismo contenido.
- Se solucionó un problema de puerta de hardware por el cual los discos virtuales QEMU se juzgaban erróneamente como SSD locales.

## Sitios y publicaciones

- Páginas de GitHub habilitadas: <https://my-god-war.github.io/eloqstore2fkv/>
- El enrutamiento del sitio admite `/eloqstore2fkv/` subrutas.
- En su lugar, el documento se lee desde `raw.githubusercontent.com` y se soluciona el 404 causado por la falta de `web/` en la ruta jsDelivr.
- La navegación del blog y la traducción multilingüe `bun run tran` están integradas; Este artículo está traducido automáticamente de la fuente china a otros idiomas.

Los registros de cambios posteriores continuarán documentando los cambios en la compatibilidad, el rendimiento y la cadena de herramientas.