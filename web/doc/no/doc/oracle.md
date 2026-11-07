# C++ oracle

`cpp/oracle` Bruk eloqstores virkelige bygger/parser for å generere bytekorpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tom / enkel / multi (plassholder) |
| store | tom / enkel / multi (plassholder) |

## grensesnitt

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

En utgangskode på 0 anses som vellykket. `verify` må sendes eksplisitt `kind` / `case`.

## Bygg avhengigheter

Linux-beholdere krever `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle vil bruke nedfiringskildekoden som er lagret i eloqstore-pinnen for å bygge et statisk bibliotek.