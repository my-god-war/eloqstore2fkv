# C++ oracle

`cpp/oracle` Koristite pravi builder/parser eloqstore za generiranje bajtova korpusa.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | prazno / jednostruko / višestruko (čuvar mjesta) |
| store | prazno / jednostruko / višestruko (čuvar mjesta) |

## interfejs

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Izlazni kod od 0 smatra se uspješnim. `verify` se mora eksplicitno proslijediti `kind` / `case`.

## Izgradite zavisnosti

Linux kontejneri zahtijevaju `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle će koristiti abseil izvorni kod pohranjen u eloqstore pin za izgradnju statičke biblioteke.