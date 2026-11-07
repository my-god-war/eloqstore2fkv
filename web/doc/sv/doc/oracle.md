# C++ oracle

`cpp/oracle` Använd eloqstores verkliga byggare/parser för att generera bytekorpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tom / enkel / multi (platshållare) |
| store | tom / enkel / multi (platshållare) |

## gränssnitt

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

En utgångskod på 0 anses vara framgångsrik. `verify` måste skickas uttryckligen `kind` / `case`.

## Bygg beroenden

Linux-behållare kräver `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle kommer att använda nedfiringskällkoden som är lagrad i eloqstore-stiftet för att bygga ett statiskt bibliotek.