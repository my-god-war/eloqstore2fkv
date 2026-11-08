# C++ oracle

`cpp/oracle` Utilizza il vero builder/parser di eloqstore per generare il corpus di byte.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | vuoto / singolo / multi (segnaposto) |
| store | vuoto / singolo / multi (segnaposto) |

## interfaccia

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Un codice di uscita pari a 0 è considerato riuscito. `verify` deve essere passato esplicitamente `kind` / `case`.

## Costruisci dipendenze

I contenitori Linux richiedono `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
<bid="1">. Oracle utilizzerà il codice sorgente abseil memorizzato nel pin eloqstore per creare una libreria statica.