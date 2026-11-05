# ຍ້າຍບັນຊີລາຍການ

`tool/fkv_ledger`ໃຊ້ tree-sitter ເພື່ອສະແກນ eloqstore, ແຜນທີ່ ແລະຢືນຢັນສັນຍາລັກ C++ ແລະ fkv
ຫຼັກຖານແມ່ນຂຽນໃສ່ motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ຖານຂໍ້ມູນບັນຊີລາຍການຢູ່ທີ່ `.data/motedb.mote` ແລະບໍ່ໄດ້ຖືກເກັບໄວ້ໃນຖານຂໍ້ມູນ.

`verified` ຕ້ອງມີຫຼັກຖານຜ່ານ; `doc/progress.md` ຖືກສ້າງຂື້ນໂດຍ `report`.