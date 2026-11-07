# C++ oracle

`cpp/oracle` Defnyddiwch adeiladwr / parser go iawn eloqstore i gynhyrchu corpws beit.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | gwag / sengl / aml (dalfan) |
| store | gwag / sengl / aml (dalfan) |

## rhyngwyneb

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Ystyrir bod cod ymadael o 0 yn llwyddiannus. Rhaid pasio `verify` yn benodol `kind` / `case`.

## Adeiladu dibyniaethau

Mae angen `cmake` `cmake`, `g++ >= 13`, `libzstd-dev`, `libgoogle-glog-dev` ar gynwysyddion Linux,
`libgflags-dev`, `libjsoncpp-dev`, `liburing-dev`, `libcurl4-openssl-dev`,
`libboost-all-dev`. Bydd Oracle yn defnyddio'r cod ffynhonnell abseil sydd wedi'i storio yn y pin eloqstore i adeiladu llyfrgell sefydlog.