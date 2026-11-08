# C++ oracle

`cpp/oracle` Za ustvarjanje bajtnega korpusa uporabite pravi graditelj/razčlenjevalnik eloqstore.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | prazno / enojno / več (ograda) |
| store | prazno / enojno / več (ograda) |

## vmesnik

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Izhodna koda 0 velja za uspešno. `verify` je treba posredovati izrecno `kind` / `case`.

## Zgradite odvisnosti

Vsebniki Linux zahtevajo `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle bo za izdelavo statične knjižnice uporabil izvorno kodo za spuščanje po vrvi, shranjeno v zatiču eloqstore.