# กระบวนการตรวจสอบ

## การทดสอบความเข้ากันได้ของสนิม

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` อาศัยรูปแบบของลัง `../fkv` ผ่านเส้นทางสัมพัทธ์ ถอดรหัสคลังข้อมูลอินพุตและทำ
การยืนยันมาตรฐานทองคำแบบไบต์ต่อไบต์

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle รวบรวมเฉพาะเลเยอร์รูปแบบ TU และไม่ต้องการ io_uring / S3 / โครงสร้างไลบรารีทั้งหมด

## คลังข้อมูล

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

ดำเนินการ `git diff --exit-code fixtures/cpp` หลังจาก CI สร้างใหม่ `fixtures/cpp` เพื่อตรวจสอบการดริฟท์

## พื้นฐาน

`COMPAT_BASELINE` ล็อคระดับการคอมมิตและความเข้ากันได้ของ eloqstore ข้อมูลพื้นฐานที่อัปเกรดแล้วจะต้องได้รับการสแกนซ้ำและยืนยันอีกครั้ง