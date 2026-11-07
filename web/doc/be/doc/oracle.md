# C++ oracle

`cpp/oracle` Выкарыстоўвайце рэальны канструктар / аналізатар eloqstore для стварэння байтавага корпуса.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | пусты / адзіночны / некалькі (запаўняльнік) |
| store | пусты / адзіночны / некалькі (запаўняльнік) |

## інтэрфейс

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Код выхаду 0 лічыцца паспяховым. `verify` павінна быць перададзена яўна `kind` / `case`.

## Стварыце залежнасці

Кантэйнеры Linux патрабуюць `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle будзе выкарыстоўваць зыходны код спуску на тросе, які захоўваецца ў PIN-кодзе eloqstore, для стварэння статычнай бібліятэкі.