# ປະສິດທິພາບ

ດາວໂຫລດ `perf-latest`ຂອງ `fkv_bench`ຈາກ fkv repository ແລະແລ່ນວຽກດຽວກັນຢູ່ໃນເຄື່ອງຄົງທີ່.
ແລະປຽບທຽບ throughput, latency quntiles, ແລະ peak RSS to a C++ baseline.

ແຫຼ່ງພາລາມິເຕີແມ່ນສອດຄ່ອງກັບ fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

ເສັ້ນພື້ນຖານ C ++ ແມ່ນດໍາເນີນການໂດຍສະພາບແວດລ້ອມ oracle `simple_bench` ແລະຈາກນັ້ນໃຫ້ຜົນໄດ້ຮັບຕາມ schema ດຽວກັນແລະຖືກເຜີຍແຜ່ໃນ
`perf-baseline` ປ່ອຍຊັບສິນ. ຜົນໄດ້ຮັບການປຽບທຽບໄດ້ຖືກຂຽນກັບຄືນໄປຫາເອກະສານສາງ, ແລະເວັບໄຊທ໌ໄດ້ສະແດງ Markdown ໂດຍກົງ.
ບໍ່ມີຊັ້ນຂໍ້ມູນ JSON ຖືກຮັກສາໄວ້.