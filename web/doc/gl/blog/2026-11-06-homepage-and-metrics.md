---
date: 2026-11-06
summary: Comparación do rendemento da páxina de inicio, expansión do indicador de avaliación, páxinas de GitHub e blogs multilingües en liña
---

# 2026-11-06: táboa de rendemento da páxina de inicio, expansión do indicador e Páxinas

A actualización de hoxe céntrase nas revisións de rendemento e nos lanzamentos de sitios.

## Comparación do rendemento da páxina de inicio

- A páxina de inicio cámbiase a prioridade de rendemento e a táboa de comparación C++/Rust móstrase directamente na primeira pantalla.
- Engádense tres novos conxuntos de indicadores: ancho de banda lóxico, retardo de cola P99.9 e amplificación espacial.
- Engadíronse detalles de execución de Rust: dixitalización, exploración completa, amplificación de escritura, número de chamadas ao sistema de escritura e sincronización de datos por lote.
- Marca explícitamente `write-read` como calibre mixto e enlaza coa documentación de rendemento.

Base actual da mesma máquina:

| índice | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS pico | 175.6 MiB | 145.5 MiB | 0.83x |
| Uso de almacenamento | 269.0 MiB | 101.4 MiB | 0.38x |

## Indicadores de avaliación

Referíndome á saída común de RocksDB, LevelDB, YCSB, WiredTiger e outros puntos de referencia, engadín:

- Ancho de banda de lectura e escritura lóxica MiB/s;
- P99.9 atraso de cola;
- Amplificación do espazo, amplificación da escritura;
- Número de chamadas ao sistema de escritura e sincronización de datos en cada lote;
- Rendemento de dixitalización/escaneo completo;
- A CPU, os fallos de páxina, os cambios de contexto e as E/S do proceso continúan recompilando o sidecar de recursos.

Xubilado ao mesmo tempo`gxhash_4096B_gib_s`. É un microbenchmark hash de bloque único de 4 KiB que non está na ruta do motor KV e non ten contrapartida en C++.

## Correccións e correccións de enxeñería

- Corrixe o espazo de teclas de `--rng cpp` lectura/escaneo: a detección de lecturas xa non se multiplica por `key_interval` para evitar que o 75 % das lecturas caia en claves non escritas.
- Corrixíronse os reintentos en espera causados ​​por macOS `openrsync` erro de xuízo de manifestos co mesmo contido.
- Solucionouse un problema de porta de hardware no que os discos virtuais QEMU foron xulgados incorrectamente como SSD locais.

## Sitios e publicacións

- Páxinas de GitHub habilitadas: <https://my-god-war.github.io/eloqstore2fkv/>
- O enrutamento do sitio admite `/eloqstore2fkv/` subrutas.
- O documento léase desde `raw.githubusercontent.com` e solucionouse o 404 causado por faltar `web/` na ruta jsDelivr.
- A navegación no blog e a `bun run tran` tradución multilingüe están integradas; este artigo tradúcese automaticamente da fonte chinés a outros idiomas.

Os rexistros de cambios posteriores seguirán documentando os cambios de compatibilidade, rendemento e cadea de ferramentas.