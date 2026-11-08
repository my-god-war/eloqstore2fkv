# C++ oracle

`cpp/oracle` Bayt korpusu yaratmaq üçün eloqstore-un real qurucusundan/parserindən istifadə edin.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | boş / tək / çox (yer tutucu) |
| store | boş / tək / çox (yer tutucu) |

## interfeys

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 çıxış kodu uğurlu sayılır. `verify` açıq şəkildə `kind` / `case` ötürülməlidir.

## Asılılıqlar qurun

Linux konteynerləri üçün `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle statik kitabxana qurmaq üçün eloqstore pinində saxlanılan abseil mənbə kodundan istifadə edəcək.