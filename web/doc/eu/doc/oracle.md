# C++ oracle

`cpp/oracle` Erabili eloqstore-ren benetako eraikitzailea / analizatzailea byte-corpusa sortzeko.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | hutsa / bakarra / anitza (leku-markoa) |
| store | hutsa / bakarra / anitza (leku-markoa) |

## interfazea

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 irteera kodea arrakastatsutzat jotzen da. `verify` esplizituki pasatu behar da `kind` / `case`.

## Mendekotasunak eraiki

Linux edukiontziek `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle-k eloqstore pinean gordetako abseil iturburu-kodea erabiliko du liburutegi estatiko bat eraikitzeko.