# C++ oracle

`cpp/oracle` Gebruik de echte bouwer / parser van eloqstore om bytecorpus te genereren.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | leeg / enkel / multi (tijdelijke aanduiding) |
| store | leeg / enkel / multi (tijdelijke aanduiding) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Een exitcode van 0 wordt als succesvol beschouwd. `verify` moet expliciet worden doorgegeven `kind` / `case`.

## Bouw afhankelijkheden op

Linux-containers vereisen `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle zal de abseil-broncode die is opgeslagen in de eloqstore-pin gebruiken om een ​​statische bibliotheek te bouwen.