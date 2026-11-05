# eloqstore2fkv

Gudang pengesahan migrasi dan keserasian dari eloqstore ke fkv.

Repositori fkv utama kekal Karat tulen dan dibina secara bebas; repositori ini mengekalkan oracle pelaksanaan rujukan C++, korpus bait dua arah,
Pindahkan lejar, rekod audit sejarah dan sahkan CI.

## kandungan

- Oracle C++: pautkan lapisan format eloqstore dan jana halaman/indeks/limpahan/manifest/
  memampatkan korpus.
- Korpus dua arah: C++ tulis → Rust read, Rust write → C++ read.
- `fkv_compat`: Ujian keserasian sisi karat, bergantung pada peti format gudang fkv.
- `fkv_ledger`: Imbas eloqstore, rekod lejar migrasi, jana dan sahkan korpus.
- Perbandingan prestasi: Bandingkan garis dasar fkv dan C++ di bawah beban kerja yang sama.

## Pintu masuk

- [Proses pengesahan](workflow.md)
- [C++ oracle](oracle.md)
- [Berhijrah lejar](ledger.md)
- [prestasi](performance.md)
- [Semak rekod](issues.md)