# ย้ายบัญชีแยกประเภท

`tool/fkv_ledger` ใช้ tree-sitter เพื่อสแกน eloqstore, map และตรวจสอบสัญลักษณ์ C++ และ fkv
หลักฐานถูกเขียนถึง motedb

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ฐานข้อมูลบัญชีแยกประเภทอยู่ที่ `.data/motedb.mote` และไม่ได้จัดเก็บไว้ในฐานข้อมูล

`verified` ต้องมีหลักฐานผ่าน; `doc/progress.md` ถูกสร้างขึ้นโดย `report`