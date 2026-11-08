# C++ oracle

`cpp/oracle` Verwenden Sie den echten Builder/Parser von eloqstore, um einen Byte-Korpus zu generieren.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | leer / einzeln / mehrfach (Platzhalter) |
| store | leer / einzeln / mehrfach (Platzhalter) |

## Schnittstelle

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Ein Exit-Code von 0 gilt als erfolgreich. `verify` muss explizit `kind` / `case` übergeben werden.

## Bauen Sie Abhängigkeiten auf

Linux-Container erfordern `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle wird den im eloqstore-Pin gespeicherten Abseil-Quellcode verwenden, um eine statische Bibliothek zu erstellen.