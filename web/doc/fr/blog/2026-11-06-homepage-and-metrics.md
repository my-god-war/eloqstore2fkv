---
date: 2026-11-06
summary: Comparaison des performances de la page d'accueil, expansion des indicateurs d'évaluation, pages GitHub et blogs multilingues en ligne
---

# 2026-11-06 : Tableau des performances de la page d'accueil, expansion des indicateurs et pages

La mise à jour d'aujourd'hui se concentre sur les évaluations de performances et les lancements de sites.

## Comparaison des performances de la page d'accueil

- La page d'accueil passe en priorité performances et le tableau de comparaison C++/Rust est directement affiché sur le premier écran.
- Trois nouveaux ensembles d'indicateurs sont ajoutés : bande passante logique, délai de queue P99.9 et amplification spatiale.
- Ajout des détails d'exécution de Rust : Scan, Fullscan, amplification d'écriture, nombre d'appels système d'écriture et synchronisation des données par lot.
- Marquez explicitement `write-read` comme calibre mixte et créez un lien vers la documentation sur les performances.

Base de référence actuelle sur la même machine :

| indice | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Pic RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Utilisation du stockage | 269.0 MiB | 101.4 MiB | 0.38x |

## Indicateurs d'évaluation

En référence à la sortie commune de RocksDB, LevelDB, YCSB, WiredTiger et d'autres benchmarks, j'ai ajouté :

- Bande passante logique de lecture et d'écriture MiB/s ;
- Délai de queue P99.9 ;
- Amplification spatiale, amplification d'écriture ;
- Nombre d'appels système d'écriture et synchronisation des données dans chaque lot ;
- Débit de numérisation/analyse complète ;
- Le processeur, les défauts de page, les changements de contexte et les E/S de processus continuent d'être collectés par le side-car de ressources.

Retraité en même temps`gxhash_4096B_gib_s`. Il s'agit d'un microbenchmark de hachage à bloc unique de 4 Ko qui ne se trouve pas sur le chemin du moteur KV et n'a pas d'équivalent C++.

## Corrections d’exactitude et d’ingénierie

- Correction de l'espace clé de `--rng cpp` read/scan : la détection de lecture n'est plus multipliée par `key_interval` pour éviter que 75 % des lectures ne tombent dans des clés non écrites.
- Correction des tentatives de mise en veille causées par macOS `openrsync` une mauvaise évaluation des manifestes avec le même contenu.
- Correction d'un problème de porte matérielle où les disques virtuels QEMU étaient considérés à tort comme des SSD locaux.

## Sites et publication

- Pages GitHub activées : <https://my-god-war.github.io/eloqstore2fkv/>
- Le routage du site prend en charge les `/eloqstore2fkv/` sous-chemins.
- Le document est lu à partir de `raw.githubusercontent.com` à la place, et le 404 provoqué par l'absence de `web/` dans le chemin jsDelivr est corrigé.
- La navigation sur le blog et la `bun run tran` traduction multilingue sont intégrées ; cet article est automatiquement traduit de la source chinoise vers d'autres langues.

Les journaux de modifications ultérieurs continueront à documenter les modifications de compatibilité, de performances et de chaîne d'outils.