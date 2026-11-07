# C++ oracle

`cpp/oracle` Brug eloqstores rigtige builder/parser til at generere bytekorpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tom / enkelt / multi (pladsholder) |
| store | tom / enkelt / multi (pladsholder) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

En udgangskode på 0 anses for vellykket. `verify` skal udtrykkeligt videregives `kind` / `case`.

## Byg afhængigheder

Linux-containere kræver `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle vil bruge nedfiringskildekoden, der er gemt i eloqstore-stiften, til at bygge et statisk bibliotek.