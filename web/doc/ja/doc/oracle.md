# C++ oracle

`cpp/oracle` eloqstore の実際のビルダー/パーサーを使用してバイト コーパスを生成します。

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | 空 / シングル / マルチ (プレースホルダー) |
| store | 空 / シングル / マルチ (プレースホルダー) |

## インタフェース

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

終了コード 0 は成功とみなされます。 `verify` は `kind` / `case` として明示的に渡す必要があります。

## 依存関係を構築する

Linux コンテナには、`cmake`、`g++ >= 13`、`libzstd-dev`、`libgoogle-glog-dev`、
`libgflags-dev`、`libjsoncpp-dev`、`liburing-dev`、`libcurl4-openssl-dev`、
`libboost-all-dev`。 Oracle は、eloqstore ピンに保存されている abseil ソース コードを使用して静的ライブラリを構築します。