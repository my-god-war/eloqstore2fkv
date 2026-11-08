# Performances

Téléchargez `perf-latest` de `fkv_bench` à partir du référentiel fkv et exécutez la même charge de travail sur une machine fixe.
Et comparez le débit, les quantiles de latence et le pic RSS à une référence C++.

Le paramètre source est cohérent avec fkv :

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

La ligne de base C++ est exécutée par l'environnement Oracle `simple_bench` puis sortie selon le même schéma et publiée sur
`perf-baseline` libérer l'élément. Les résultats de la comparaison sont réécrits dans le document d'entrepôt et le site restitue directement Markdown.
Aucune couche de données JSON n'est conservée.

## Résultats actuels

schéma de référence 2 pour 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, page de 4 Ko,
`data_append_mode=false`, `direct_io=true`. Charge de travail fixe :
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projet | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Mettre le débit | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Obtenez du débit | 5,526 QPS | 7,293 QPS | 1.32x |
| Mettre de la bande passante logique | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Obtenez une bande passante logique | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Pic RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Utilisation du stockage | 269.0 MiB | 101.4 MiB | 0.38x |
| Stockage / données nominales | 2.20x | 0.83x | 0.38x |

L'écriture de 1 million d'entrées (10 000 lots) correspond à
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, c'est-à-dire que les données sont synchronisées une fois par lot.
Métriques supplémentaires Rust : Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, Write Amplification `2.57x`
(`proc_io_write` 329 Mo/128 Mo d'écritures logiques), `4.86` écriture des appels système par lot.
Le taux de stockage est calculé sur la base de 1M × 128B = 128 Mo de données nominales ; C++ ne déclenche pas GC/compact cette fois,
Il contient donc plus de pages d’histoire. Ce n'est pas le même numéro de calibre que « l'amplification spatiale des données de survie ».
La latence de queue P99.9 de Rust est encore pire que celle de C++ (mettez 223,3 ms contre 86,0 ms, obtenez 355,5 ms contre
48,2 ms), la ligne de base unique fluctue considérablement, veuillez consulter le rapport de performances interne pour plus de détails.

**Remarque** : `write-read` de C++ signifie que le thread d'écriture et le thread de lecture s'exécutent en même temps, et Rust actuellement
"Terminer l'écriture, rouvrir et relire", il n'y a pas de concours d'écriture au stade de la lecture. Par conséquent, le multiplicateur de `write-read` ne peut pas être traité comme
Lecture pure / conclusion pure écriture ; une comparaison stricte devrait voir `scripts/perf_local.sh --symmetric`
Deux paires de résultats sont générées : en écriture seule et en lecture seule. Les numéros de coureur partagés uniques ne sont utilisés que pour la régression
Les tendances et les sorties sont jugées en utilisant la médiane de `--runs`.