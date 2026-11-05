# C++ oracle

`cpp/oracle` ใช้ตัวสร้าง / ตัวแยกวิเคราะห์ที่แท้จริงของ eloqstore เพื่อสร้างคลังข้อมูลไบต์

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ว่าง / เดี่ยว / หลาย (ตัวยึดตำแหน่ง) |
| store | ว่าง / เดี่ยว / หลาย (ตัวยึดตำแหน่ง) |

## อินเตอร์เฟซ

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

รหัสทางออกเป็น 0 ถือว่าสำเร็จ `verify` จะต้องส่งผ่านอย่างชัดเจน `kind` / `case`

## สร้างการพึ่งพา

คอนเทนเนอร์ Linux ต้องการ `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle จะใช้ซอร์สโค้ด abseil ที่จัดเก็บไว้ในพิน eloqstore เพื่อสร้างไลบรารีแบบคงที่