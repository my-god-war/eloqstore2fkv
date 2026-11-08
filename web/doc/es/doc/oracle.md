# C++ oracle

`cpp/oracle` Utilice el generador/analizador real de eloqstore para generar un corpus de bytes.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | vacío / único / múltiple (marcador de posición) |
| store | vacío / único / múltiple (marcador de posición) |

## interfaz

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Un código de salida de 0 se considera exitoso. `verify` debe pasarse explícitamente `kind`/`case`.

## Construir dependencias

Los contenedores de Linux requieren `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle utilizará el código fuente de rappel almacenado en el pin eloqstore para construir una biblioteca estática.