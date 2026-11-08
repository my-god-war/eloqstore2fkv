# Rendiment

Baixeu `perf-latest` de `fkv_bench` des del repositori fkv i executeu la mateixa càrrega de treball en una màquina fixa.
I compareu el rendiment, els quantils de latència i l'RSS màxim amb una línia de base C++.

La font del paràmetre és coherent amb fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

L'entorn d'oracle `simple_bench` executa la línia base de C++ i després es publica segons el mateix esquema i es publica a
`perf-baseline` allibera el recurs. Els resultats de la comparació es tornen a escriure al document de magatzem i el lloc representa directament Markdown.
No es manté cap capa de dades JSON.

## Resultats actuals

L'esquema 2 de referència per a 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, pàgina de 4 KiB,
`data_append_mode=false`, `direct_io=true`. Càrrega de treball fixada:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projecte | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Posa el rendiment | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Obteniu rendiment | 5,526 QPS | 7,293 QPS | 1.32x |
| Posa ample de banda lògic | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Obteniu ample de banda lògic | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS màxim | 175.6 MiB | 145.5 MiB | 0.83x |
| Ús d'emmagatzematge | 269.0 MiB | 101.4 MiB | 0.38x |
| Emmagatzematge / dades nominals | 2.20x | 0.83x | 0.38x |

Escriure 1 milió d'entrades (10.000 lots) correspon a
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, és a dir, les dades es sincronitzen una vegada per lot.
Mètriques addicionals d'oxidació: escaneig `141,166 kv/s`, exploració completa `108,274 kv/s`, amplificació d'escriptura `2.57x`
(`proc_io_write` 329 MB / 128 MB d'escriptures lògiques), `4.86` escriviu trucades al sistema per lot.
La relació d'emmagatzematge es calcula a partir d'1M × 128B = 128 MB de dades nominals; C++ no activa GC/compact aquesta vegada,
Així que conté més pàgines d'història. No és el mateix número de calibre que "amplificació de l'espai de dades de supervivència".
La latència de cua P99.9 de Rust encara és pitjor que C++ (posar 223,3 ms vs 86,0 ms, obtenir 355,5 ms vs
48,2 ms), la línia de base única fluctua molt; consulteu l'informe de rendiment intern per obtenir-ne més detalls.

**Nota**: `write-read` de C++ significa que el fil d'escriptura i el fil de lectura s'executen al mateix temps, i Rust actualment
"Completa l'escriptura, torna a obrir i torna a llegir", no hi ha concurs d'escriptura a l'etapa de lectura. Per tant, el multiplicador de `write-read` no es pot tractar com
Pura lectura / pura escriptura conclusió; una comparació estricta hauria de veure `scripts/perf_local.sh --symmetric`
Es produeixen dos parells de resultats: només d'escriptura i només de lectura. Els números de corredor compartit únic només s'utilitzen per a la regressió
Les tendències i els llançaments es valoren utilitzant la mediana de `--runs`.