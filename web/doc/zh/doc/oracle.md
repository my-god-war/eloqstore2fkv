# C++ oracle

`cpp/oracle` 用 eloqstore 真实的 builder / parser 生成字节语料。

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | empty / single / multi（占位） |
| store | empty / single / multi（占位） |

## 接口

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

退出码 0 视为成功。`verify` 必须显式传 `kind` / `case`。

## 构建依赖

Linux 容器需要 `cmake`、`g++ >= 13`、`libzstd-dev`、`libgoogle-glog-dev`、
`libgflags-dev`、`libjsoncpp-dev`、`liburing-dev`、`libcurl4-openssl-dev`、
`libboost-all-dev`。oracle 会用 eloqstore pin 住的 abseil 源码构建静态库。
