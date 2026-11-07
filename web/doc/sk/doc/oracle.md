# C++ oracle

`cpp/oracle` Na generovanie bajtového korpusu použite skutočný zostavovač / syntaktický analyzátor eloqstore.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | prázdny / jeden / viac (zástupný symbol) |
| store | prázdny / jeden / viac (zástupný symbol) |

## rozhranie

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Výstupný kód 0 sa považuje za úspešný. `verify` musí byť odovzdané explicitne `kind` / `case`.

## Budujte závislosti

Kontajnery Linux vyžadujú `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle použije zdrojový kód zlaňovania uložený v kolíku eloqstore na vytvorenie statickej knižnice.