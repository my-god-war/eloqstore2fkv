# C++ oracle

`cpp/oracle` Käytä eloqstoren todellista rakentajaa/jäsentäjää tavukorpuksen luomiseen.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tyhjä / yksittäinen / moni (paikkamerkki) |
| store | tyhjä / yksittäinen / moni (paikkamerkki) |

## käyttöliittymä

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Poistumiskoodi 0 katsotaan onnistuneeksi. `verify` on välitettävä eksplisiittisesti `kind` / `case`.

## Rakenna riippuvuuksia

Linux-säilöt vaativat `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle käyttää eloqstore-pintaan tallennettua abseil-lähdekoodia staattisen kirjaston rakentamiseen.