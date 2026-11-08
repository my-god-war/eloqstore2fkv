# C++ oracle

`cpp/oracle`ໃຊ້ builder / parser ທີ່ແທ້ຈິງຂອງ eloqstore ເພື່ອສ້າງ byte corpus.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ຫວ່າງເປົ່າ / ດຽວ / ຫຼາຍ (ບ່ອນວາງ) |
| store | ຫວ່າງເປົ່າ / ດຽວ / ຫຼາຍ (ບ່ອນວາງ) |

## ການໂຕ້ຕອບ

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

ລະຫັດອອກຂອງ 0 ແມ່ນຖືວ່າປະສົບຜົນສໍາເລັດ. `verify` ຕ້ອງຜ່ານຢ່າງຈະແຈ້ງ `kind` / `case`.

## ສ້າງຄວາມເພິ່ງພາອາໄສ

ກ່ອງບັນຈຸ Linux ຕ້ອງການ `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle ຈະໃຊ້ລະຫັດແຫຼ່ງ abseil ທີ່ເກັບໄວ້ໃນ pin eloqstore ເພື່ອສ້າງຫ້ອງສະຫມຸດຄົງທີ່.