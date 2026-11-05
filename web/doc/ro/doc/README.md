# eloqstore2fkv

Depozitul de verificare a migrației și a compatibilității de la eloqstore la fkv.

Depozitul principal fkv rămâne pur Rust și este construit independent; acest depozit păstrează oracolul de implementare de referință C++, corpus de octeți bidirecționali,
Migrați registrele, înregistrările istorice de audit și verificați CI.

## conţinut

- Oracol C++: conectează stratul de format eloqstore și generează pagina/index/overflow/manifest/
  comprima corpus.
- Corpus bidirecțional: C++ scriere → Rust citire, Rust scriere → C++ citire.
- `fkv_compat`: test de compatibilitate cu partea ruginii, bazându-se pe cutia de format a depozitului fkv.
- `fkv_ledger`: Scanați eloqstore, înregistrați registrul de migrare, generați și verificați corpus.
- Comparație de performanță: comparați liniile de bază fkv și C++ sub aceeași sarcină de lucru.

## Intrare

- [Proces de verificare](workflow.md)
- [C++ oracle](oracle.md)
- [Migrați registrul](ledger.md)
- [performanţă](performance.md)
- [Revizuiți înregistrările](issues.md)