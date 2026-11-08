# C++ oracle

`cpp/oracle` Benotzt eloqstore's real Builder / Parser fir Byte Corpus ze generéieren.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | eidel / eenzel / Multi (Plazhalter) |
| store | eidel / eenzel / Multi (Plazhalter) |

## Interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

En Ausgangscode vun 0 gëtt als erfollegräich ugesinn. `verify` muss explizit passéiert ginn `kind` / `case`.

## Ofhängegkeeten bauen

Linux Container erfuerderen `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle wäert d'abseil Quelltext benotzen am eloqstore PIN gespäichert fir eng bauen eng statesch Bibliothéik.