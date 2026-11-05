# C++ oracle

`cpp/oracle` Izmantojiet eloqstore reālo veidotāju/parsētāju, lai ģenerētu baitu korpusu.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tukšs / viens / vairāki (vietturis) |
| store | tukšs / viens / vairāki (vietturis) |

## saskarne

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Izejas kods 0 tiek uzskatīts par veiksmīgu. `verify` ir skaidri jānodod `kind` / `case`.

## Veidojiet atkarības

Linux konteineriem ir nepieciešami `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle izmantos abseil avota kodu, kas saglabāts eloqstore tapā, lai izveidotu statisku bibliotēku.