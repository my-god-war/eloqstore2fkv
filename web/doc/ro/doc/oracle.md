# C++ oracle

`cpp/oracle` Utilizați generatorul/parserul real al eloqstore pentru a genera corpus de octeți.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | gol / singur / multiplu (substituent) |
| store | gol / singur / multiplu (substituent) |

## interfata

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Un cod de ieșire de 0 este considerat reușit. `verify` trebuie transmis în mod explicit `kind` / `case`.

## Construiți dependențe

Containerele Linux necesită `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle va folosi codul sursă de abseil stocat în pin-ul eloqstore pentru a construi o bibliotecă statică.