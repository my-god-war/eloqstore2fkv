# C++ oracle

`cpp/oracle` Kasutage baitkorpuse genereerimiseks eloqstore'i tegelikku ehitajat/parserit.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tühi / üksik / mitu (kohahoidja) |
| store | tühi / üksik / mitu (kohahoidja) |

## liides

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Väljumiskoodi 0 loetakse edukaks. `verify` tuleb selgesõnaliselt edastada `kind` / `case`.

## Loo sõltuvused

Linuxi konteinerite jaoks on vaja `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle kasutab staatilise teegi koostamiseks eloqstore'i tihvti salvestatud abseil-lähtekoodi.