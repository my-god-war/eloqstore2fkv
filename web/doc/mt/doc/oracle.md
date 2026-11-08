# C++ oracle

`cpp/oracle` Uża l-bennej/parser reali ta' eloqstore biex tiġġenera byte corpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | vojta / waħda / multi (placeholder) |
| store | vojta / waħda / multi (placeholder) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Kodiċi ta' ħruġ ta' 0 jitqies bħala suċċess. `verify` trid tiġi mgħoddija espliċitament `kind` / `case`.

## Ibni dipendenzi

Kontenituri Linux jeħtieġu `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. L-Oracle se juża l-kodiċi tas-sors tal-abseil maħżun fil-pin eloqstore biex jibni librerija statika.