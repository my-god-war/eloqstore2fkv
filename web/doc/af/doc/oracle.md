# C++ oracle

`cpp/oracle` Gebruik eloqstore se regte bouer/ontleder om greepkorpus te genereer.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | leeg / enkel / multi (plekhouer) |
| store | leeg / enkel / multi (plekhouer) |

## koppelvlak

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

'n Uittreekode van 0 word as suksesvol beskou. `verify` moet eksplisiet geslaag word `kind` / `case`.

## Bou afhanklikhede

Linux-houers vereis `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle sal die abseil-bronkode wat in die eloqstore-pen gestoor is, gebruik om 'n statiese biblioteek te bou.