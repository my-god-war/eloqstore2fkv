# C++ oracle

`cpp/oracle` Gunakan pembina / penghurai sebenar eloqstore untuk menjana korpus bait.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | kosong / tunggal / berbilang (tempat letak) |
| store | kosong / tunggal / berbilang (tempat letak) |

## antara muka

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Kod keluar 0 dianggap berjaya. `verify` mesti dihantar secara eksplisit `kind` / `case`.

## Bina kebergantungan

Bekas Linux memerlukan `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle akan menggunakan kod sumber abseil yang disimpan dalam pin eloqstore untuk membina perpustakaan statik.