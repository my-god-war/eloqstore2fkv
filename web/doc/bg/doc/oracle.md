# C++ oracle

`cpp/oracle` Използвайте реалния конструктор/парсер на eloqstore, за да генерирате байтов корпус.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | празно/единично/много (заместител) |
| store | празно/единично/много (заместител) |

## интерфейс

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Изходен код 0 се счита за успешен. `verify` трябва да се предаде изрично `kind` / `case`.

## Изграждане на зависимости

Linux контейнерите изискват `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle ще използва изходния код за спускане с въже, съхранен в щифта на eloqstore, за да изгради статична библиотека.