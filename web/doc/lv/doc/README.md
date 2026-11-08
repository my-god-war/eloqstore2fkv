# eloqstore2fkv

Migrācijas un saderības pārbaudes noliktava no eloqstore uz fkv.

Galvenā fkv krātuve paliek tīra rūsa un tiek būvēta neatkarīgi; šajā repozitorijā tiek saglabāts C++ atsauces ieviešanas orākuls, divvirzienu baitu korpuss,
Migrējiet virsgrāmatas, vēsturiskos audita ierakstus un pārbaudiet CI.

## saturu

- C++ Oracle: saistiet eloqstore formāta slāni un ģenerējiet lapu/index/overflow/manifest/
  saspiest korpusu.
- Divvirzienu korpuss: C++ rakstīšana → Rust lasīšana, Rust rakstīšana → C++ lasīšana.
- `fkv_compat`: rūsas puses saderības pārbaude, paļaujoties uz fkv noliktavas formāta kasti.
- `fkv_ledger`: skenējiet eloqstore, ierakstiet migrācijas virsgrāmatu, ģenerējiet un pārbaudiet korpusu.
- Veiktspējas salīdzinājums: salīdziniet fkv un C++ bāzes līnijas ar tādu pašu darba slodzi.

## Ieeja

- [Verifikācijas process](workflow.md)
- [C++ oracle](oracle.md)
- [Migrēt virsgrāmatu](ledger.md)
- [sniegumu](performance.md)
- [Pārskatiet ierakstus](issues.md)