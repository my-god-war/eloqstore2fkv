# C++ oracle

`cpp/oracle` Za generiranje korpusa bajtova upotrijebite eloqstoreov pravi alat za izgradnju/parser.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | prazno / jedno / više (rezervirano mjesto) |
| store | prazno / jedno / više (rezervirano mjesto) |

## sučelje

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Izlazni kod 0 smatra se uspješnim. `verify` mora se proslijediti eksplicitno `kind` / `case`.

## Izgradite ovisnosti

Linux spremnici zahtijevaju `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle će koristiti izvorni kod za spuštanje na užetu pohranjen u eloqstore pinu za izgradnju statičke knjižnice.