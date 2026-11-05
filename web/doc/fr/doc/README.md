# eloqstore2fkv

Entrepôt de migration et de vérification de compatibilité d'eloqstore vers fkv.

Le référentiel fkv principal reste pur Rust et est construit indépendamment ; ce référentiel conserve l'oracle d'implémentation de référence C++, le corpus d'octets bidirectionnel,
Migrez les grands livres, les enregistrements d'audit historiques et vérifiez le CI.

## contenu

- Oracle C++ : lier la couche de format eloqstore et générer page/index/overflow/manifest/
  compresser le corpus.
- Corpus bidirectionnel : écriture C++ → lecture Rust, écriture Rust → lecture C++.
- `fkv_compat` : Test de compatibilité côté Rust, s'appuyant sur le format crate de l'entrepôt fkv.
- `fkv_ledger` : analysez eloqstore, enregistrez le grand livre de migration, générez et vérifiez le corpus.
- Comparaison des performances : comparez les lignes de base fkv et C++ sous la même charge de travail.

## Entrée

- [Processus de vérification](workflow.md)
- [C++ oracle](oracle.md)
- [Migrer le grand livre](ledger.md)
- [performance](performance.md)
- [Examiner les dossiers](issues.md)