# C++ oracle

`cpp/oracle` Bayt korpusini yaratish uchun eloqstore-ning haqiqiy quruvchisi/parseridan foydalaning.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | bo'sh / bitta / ko'p (to'ldiruvchi) |
| store | bo'sh / bitta / ko'p (to'ldiruvchi) |

## interfeys

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Chiqish kodi 0 muvaffaqiyatli hisoblanadi. `verify` aniq o'tkazilishi kerak `kind` / `case`.

## Bog'liqlarni yaratish

Linux konteynerlari uchun `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle statik kutubxona yaratish uchun eloqstore pinida saqlangan abseil manba kodidan foydalanadi.