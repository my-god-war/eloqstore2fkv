# C++ oracle

`cpp/oracle` Gamita ang tinuod nga tigtukod / parser sa eloqstore aron makamugna og byte corpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | walay sulod / single / multi (placeholder) |
| store | walay sulod / single / multi (placeholder) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Ang exit code nga 0 giisip nga malampuson. Ang `verify` kinahanglang ipasa nga klaro `kind` / `case`.

## Pagtukod og mga dependency

Ang mga sudlanan sa Linux nanginahanglan `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Gagamiton sa Oracle ang abseil source code nga gitipigan sa eloqstore pin para magtukod ug static library.