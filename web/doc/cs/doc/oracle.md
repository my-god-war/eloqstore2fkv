# C++ oracle

`cpp/oracle` Ke generování bajtového korpusu použijte skutečný builder / parser eloqstore.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | prázdný / jeden / více (zástupný symbol) |
| store | prázdný / jeden / více (zástupný symbol) |

## rozhraní

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Výstupní kód 0 je považován za úspěšný. `verify` musí být předáno explicitně `kind` / `case`.

## Vytvářejte závislosti

Kontejnery Linux vyžadují `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle použije zdrojový kód slaňování uložený na pinu eloqstore k vytvoření statické knihovny.