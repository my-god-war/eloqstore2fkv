# C++ oracle

`cpp/oracle` Використовуйте справжній конструктор/аналізатор eloqstore, щоб створити байтовий корпус.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | порожній / одиночний / багаторазовий (заповнювач) |
| store | порожній / одиночний / багаторазовий (заповнювач) |

## інтерфейс

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Код виходу 0 вважається успішним. `verify` необхідно передати явно `kind` / `case`.

## Створіть залежності

Для контейнерів Linux потрібні `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle використовуватиме вихідний код спуску на тросі, що зберігається в PIN-коді eloqstore, для створення статичної бібліотеки.