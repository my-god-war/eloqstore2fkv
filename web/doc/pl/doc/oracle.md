# C++ oracle

`cpp/oracle` Użyj prawdziwego konstruktora/parsera eloqstore do wygenerowania korpusu bajtów.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | pusty / pojedynczy / wielokrotny (symbol zastępczy) |
| store | pusty / pojedynczy / wielokrotny (symbol zastępczy) |

## interfejs

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Kod wyjścia o wartości 0 uznaje się za udany. `verify` należy przekazać jawnie `kind` / `case`.

## Buduj zależności

Kontenery Linux wymagają `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle użyje kodu źródłowego abseil zapisanego w pinie eloqstore do zbudowania biblioteki statycznej.