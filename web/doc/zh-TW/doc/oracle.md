# C++ oracle

`cpp/oracle` 用 eloqstore 真實的 builder / parser 產生位元組語料。

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | empty / single / multi（佔位） |
| store | empty / single / multi（佔位） |

## 介面

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

退出碼 0 視為成功。 `verify` 必須明確傳 `kind` / `case`。

## 建構依賴

Linux 容器需 `cmake`、`g++ >= 13`、`libzstd-dev`、`libgoogle-glog-dev`、
`libgflags-dev`、`libjsoncpp-dev`、`liburing-dev`、`libcurl4-openssl-dev`、
`libboost-all-dev`。 oracle 會用 eloqstore pin 住的 abseil 原始碼建構靜態函式庫。