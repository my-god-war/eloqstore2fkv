# C++ oracle

`cpp/oracle` Используйте настоящий конструктор/парсер eloqstore для создания корпуса байтов.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | пустой/одиночный/множественный (заполнитель) |
| store | пустой/одиночный/множественный (заполнитель) |

## интерфейс

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Код выхода 0 считается успешным. `verify` должен быть передан явно `kind` / `case`.

## Создание зависимостей

Для контейнеров Linux требуются `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle будет использовать исходный код abseil, хранящийся в выводе eloqstore, для создания статической библиотеки.