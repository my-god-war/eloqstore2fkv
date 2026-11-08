# Examiner les dossiers

`issue/` est le comité de révision pendant la migration :

- `new/` : entrée pour les nouveaux commentaires.
- `todo/` : Adopté, en attente de développement.
- `done/` : développé et dispose de preuves de test, en attente d'examen.
- `reviewed/` : Archive examinée.
- `reject/` : En cas de non-adoption, veuillez en indiquer les raisons.

Voir `issue/new.md`, `issue/run.md`, `issue/review.md` pour le processus.

L'examen se concentre sur les différences de comportement, la compatibilité des formats, les performances et la structure du code entre les implémentations fkv et C++. Un avis par
fichiers pour éviter les déplacements massifs.