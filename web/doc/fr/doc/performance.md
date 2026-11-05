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