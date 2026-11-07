# C++ oracle

`cpp/oracle` Utilisez le véritable constructeur/analyseur d'eloqstore pour générer un corpus d'octets.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | vide / simple / multi (espace réservé) |
| store | vide / simple / multi (espace réservé) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Un code de sortie de 0 est considéré comme réussi. `verify` doit être transmis explicitement `kind` / `case`.

## Construire des dépendances

Les conteneurs Linux nécessitent `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle utilisera le code source de rappel stocké dans la broche eloqstore pour créer une bibliothèque statique.