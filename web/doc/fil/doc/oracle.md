# C++ oracle

`cpp/oracle` Gamitin ang tunay na tagabuo / parser ng eloqstore upang bumuo ng byte corpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | walang laman / single / marami (placeholder) |
| store | walang laman / single / marami (placeholder) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Ang isang exit code na 0 ay itinuturing na matagumpay. Ang `verify` ay dapat na tahasang maipasa `kind` / `case`.

## Bumuo ng mga dependency

Ang mga lalagyan ng Linux ay nangangailangan ng `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Gagamitin ng Oracle ang abseil source code na nakaimbak sa eloqstore pin upang bumuo ng isang static na library.