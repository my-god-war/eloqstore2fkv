# C++ oracle

`cpp/oracle` Користете го вистинскиот градител/парсер на eloqstore за да генерирате корпус на бајти.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | празно / единечно / повеќе (место) |
| store | празно / единечно / повеќе (место) |

## интерфејс

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Излезната шифра 0 се смета за успешна. `verify` мора да се пренесе експлицитно `kind` / `case`.

## Изградете зависности

Контејнерите на Linux бараат `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle ќе го користи изворниот код на abseil зачуван во пинот на eloqstore за да изгради статична библиотека.