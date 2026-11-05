# C++ oracle

`cpp/oracle` Bayt kümesi oluşturmak için eloqstore'un gerçek oluşturucusunu/ayrıştırıcısını kullanın.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | boş / tekli / çoklu (yer tutucu) |
| store | boş / tekli / çoklu (yer tutucu) |

## arayüz

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 çıkış kodu başarılı kabul edilir. `verify` açıkça iletilmelidir `kind` / `case`.

## Bağımlılıklar oluşturun

Linux kapsayıcıları `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev` gerektirir
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle, statik bir kitaplık oluşturmak için eloqstore pininde saklanan abseil kaynak kodunu kullanacaktır.