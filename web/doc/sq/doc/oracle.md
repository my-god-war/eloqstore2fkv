# C++ oracle

`cpp/oracle` Përdorni ndërtuesin / analizuesin real të eloqstore për të gjeneruar korpus bajt.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | bosh / e vetme / e shumëfishtë (mbajtës vendi) |
| store | bosh / e vetme / e shumëfishtë (mbajtës vendi) |

## ndërfaqe

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Një kod daljeje prej 0 konsiderohet i suksesshëm. `verify` duhet të kalohet në mënyrë eksplicite `kind` / `case`.

## Ndërtoni varësi

Kontejnerët Linux kërkojnë `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle do të përdorë kodin burimor abseil të ruajtur në pinin eloqstore për të ndërtuar një bibliotekë statike.