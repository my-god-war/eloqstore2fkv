# eloqstore2fkv

Migrasie en verenigbaarheid verifikasie pakhuis van eloqstore na fkv.

Die hoof fkv-bewaarplek bly suiwer Roes en is onafhanklik gebou; hierdie bewaarplek behou die C++ verwysingsimplementeringsorakel, tweerigtinggreepkorpus,
Migreer grootboeke, historiese ouditrekords en verifieer CI.

## inhoud

- C++ oracle: skakel eloqstore-formaatlaag en genereer bladsy/indeks/oorloop/manifest/
  komprimeer korpus.
- Tweerigtingkorpus: C++ skryf → Roes lees, Roes skryf → C++ lees.
- `fkv_compat`: Roeskant-versoenbaarheidstoets, wat staatmaak op die formaatkrat van die fkv-pakhuis.
- `fkv_ledger`: Skandeer eloqstore, teken migrasiegrootboek aan, genereer en verifieer korpus.
- Prestasievergelyking: Vergelyk fkv en C++ basislyne onder dieselfde werklading.

## Ingang

- [Verifikasie proses](workflow.md)
- [C++ oracle](oracle.md)
- [Migreer grootboek](ledger.md)
- [prestasie](performance.md)
- [Hersien rekords](issues.md)