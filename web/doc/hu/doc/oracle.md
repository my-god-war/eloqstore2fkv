# C++ oracle

`cpp/oracle` Használja az eloqstore valódi építőjét / elemzőjét a bájt korpusz létrehozásához.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | üres / egyetlen / több (helyőrző) |
| store | üres / egyetlen / több (helyőrző) |

## felület

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

A 0-s kilépési kód sikeresnek tekinthető. `verify` kifejezetten `kind` / `case` kell átadni.

## Építsen függőséget

A Linux-tárolókhoz `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Az Oracle az eloqstore pinben tárolt abseil forráskódot fogja használni egy statikus könyvtár létrehozásához.