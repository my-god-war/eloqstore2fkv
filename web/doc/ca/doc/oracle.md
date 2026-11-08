# C++ oracle

`cpp/oracle` Utilitzeu el generador/analitzador real d'eloqstore per generar un corpus de bytes.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | buit / únic / múltiple (marcador de posició) |
| store | buit / únic / múltiple (marcador de posició) |

## interfície

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Un codi de sortida de 0 es considera correcte. `verify` s'ha de passar explícitament `kind` / `case`.

## Construir dependències

Els contenidors Linux requereixen `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle utilitzarà el codi font de ràpel emmagatzemat al pin eloqstore per crear una biblioteca estàtica.