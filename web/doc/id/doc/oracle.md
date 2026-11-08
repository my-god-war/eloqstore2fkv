# C++ oracle

`cpp/oracle` Gunakan pembuat/parser asli eloqstore untuk menghasilkan korpus byte.

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

Kode keluar 0 dianggap berhasil. `verify` harus diteruskan secara eksplisit `kind` / `case`.

## Bangun ketergantungan

Kontainer Linux memerlukan `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle akan menggunakan kode sumber abseil yang disimpan di pin eloqstore untuk membangun perpustakaan statis.