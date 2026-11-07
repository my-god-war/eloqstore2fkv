# C++ oracle

`cpp/oracle` Gunakake pembangun / parser nyata eloqstore kanggo ngasilake byte corpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | kosong / tunggal / multi (placeholder) |
| store | kosong / tunggal / multi (placeholder) |

## antarmuka

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Kode metu 0 dianggep sukses. `verify` kudu diterusake kanthi eksplisit `kind` / `case`.

## Mbangun dependensi

Wadah Linux mbutuhake `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle bakal nggunakake kode sumber abseil disimpen ing pin eloqstore kanggo mbangun perpustakaan statis.