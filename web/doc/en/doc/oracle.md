# C++ oracle

`cpp/oracle` Use eloqstore's real builder / parser to generate byte corpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | empty / single / multi (placeholder) |
| store | empty / single / multi (placeholder) |

## Interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

An exit code of 0 is considered successful. `verify` must be passed explicitly `kind` / `case`.

## Build dependencies

Linux containers require `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle will use the abseil source code stored in the eloqstore pin to build a static library.