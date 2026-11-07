# eloqstore2fkv

Magazina e verifikimit të migrimit dhe përputhshmërisë nga eloqstore në fkv.

Depoja kryesore e fkv mbetet Rust e pastër dhe është ndërtuar në mënyrë të pavarur; kjo depo ruan orakullin e zbatimit të referencës C++, korpusin e bajtit dydrejtimësh,
Migroni regjistrat, të dhënat historike të auditimit dhe verifikoni CI.

## përmbajtjen

- Orakulli C++: lidhni shtresën e formatit të eloqstore dhe gjeneroni faqe/indeks/mbushje/manifest/
  ngjesh korpusin.
- Korpusi dydrejtimësh: C++ shkruaj → Rust read, Rust write → C++ read.
- `fkv_compat`: Testi i përputhshmërisë nga ana e ndryshkut, duke u mbështetur në arkën e formatit të magazinës fkv.
- `fkv_ledger`: Skanoni eloqstore, regjistroni librin e migracionit, krijoni dhe verifikoni korpusin.
- Krahasimi i performancës: Krahasoni linjat bazë fkv dhe C++ nën të njëjtën ngarkesë pune.

## Hyrja

- [Procesi i verifikimit](workflow.md)
- [C++ oracle](oracle.md)
- [Migrimi i librit](ledger.md)
- [performancës](performance.md)
- [Rishikoni të dhënat](issues.md)