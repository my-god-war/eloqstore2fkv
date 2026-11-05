# C++ oracle

`cpp/oracle` Use o creador/analizador real de eloqstore para xerar corpus de bytes.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | baleiro / único / múltiple (marcador de posición) |
| store | baleiro / único / múltiple (marcador de posición) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Un código de saída de 0 considérase exitoso. `verify` debe pasarse explícitamente `kind` / `case`.

## Construír dependencias

Os contedores de Linux requiren `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle usará o código fonte de rappel almacenado no pin eloqstore para construír unha biblioteca estática.