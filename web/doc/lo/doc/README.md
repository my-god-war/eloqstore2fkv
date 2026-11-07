# eloqstore2fkv

ການຍ້າຍຖິ່ນຖານ ແລະການຢັ້ງຢືນຄວາມເຂົ້າກັນໄດ້ຈາກ eloqstore ໄປ fkv.

ການເກັບຮັກສາ fkv ຕົ້ນຕໍຍັງຄົງເປັນ Rust ບໍລິສຸດແລະຖືກສ້າງຂຶ້ນຢ່າງເປັນເອກະລາດ; repository ນີ້ຮັກສາການປະຕິບັດການອ້າງອິງ C ++ oracle, bidirectional byte corpus,
ຍ້າຍບັນຊີລາຍການ, ບັນທຶກການກວດສອບປະຫວັດສາດ ແລະກວດສອບ CI.

## ເນື້ອໃນ

- C++ oracle: ເຊື່ອມຕໍ່ຊັ້ນຮູບແບບ eloqstore ແລະສ້າງຫນ້າ/index/overflow/manifest/
  ບີບອັດ corpus.
- Bidirectional corpus: C++ write → Rust read, Rust write → C++ read.
- `fkv_compat`: ການທົດສອບຄວາມເຂົ້າກັນໄດ້ດ້ານສະນິມ, ອີງໃສ່ຮູບແບບຂອງຄັງສິນຄ້າ fkv.
- `fkv_ledger`: ສະແກນ eloqstore, ບັນທຶກບັນຊີການເຄື່ອນຍ້າຍ, ສ້າງ ແລະກວດສອບ corpus.
- ການປຽບທຽບປະສິດທິພາບ: ປຽບທຽບ fkv ແລະ C++ baselines ພາຍໃຕ້ວຽກດຽວກັນ.

## ທາງເຂົ້າ

- [ຂະບວນການຢັ້ງຢືນ](workflow.md)
- [C++ oracle](oracle.md)
- [ຍ້າຍບັນຊີລາຍການ](ledger.md)
- [ການປະຕິບັດ](performance.md)
- [ທົບທວນບັນທຶກ](issues.md)