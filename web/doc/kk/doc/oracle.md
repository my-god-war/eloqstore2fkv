# C++ oracle

`cpp/oracle` Байт корпусын жасау үшін eloqstore-ның нақты құрастырушысын/талдаушысын пайдаланыңыз.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | бос / жалғыз / көп (толтырғыш) |
| store | бос / жалғыз / көп (толтырғыш) |

## интерфейс

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 шығу коды сәтті деп саналады. `verify` анық `kind` / `case` берілуі керек.

## Тәуелділіктерді құру

Linux контейнерлері үшін `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle статикалық кітапхананы құру үшін eloqstore түйреуішінде сақталған abseil бастапқы кодын пайдаланады.