# eloqstore2fkv

Migrazio eta bateragarritasuna egiaztatzeko biltegia eloqstore-tik fkv-ra.

Fkv biltegi nagusiak Rust hutsa izaten jarraitzen du eta modu independentean eraikitzen da; Biltegi honek C++ erreferentziaren inplementazio-orakulua gordetzen du, bi norabideko byte-corpusa,
Migratu liburuak, auditoretza-erregistro historikoak eta egiaztatu CI.

## edukia

- C++ orakulua: lotu eloqstore formatuaren geruza eta sortu orria/indizea/gainezkatzea/manifestua/
  konprimitu corpusa.
- Norabide biko corpusa: C++ idatzi → Rust irakurri, Rust idatzi → C++ irakurri.
- `fkv_compat`: Herdoilaren aldean bateragarritasun-proba, fkv biltegiaren formatu-kaxaan oinarrituta.
- `fkv_ledger`: Eskaneatu eloqstore, erregistratu migrazio liburua, sortu eta egiaztatu corpusa.
- Errendimenduaren konparazioa: konparatu fkv eta C++ oinarri-lerroak lan-karga berdinean.

## Sarrera

- [Egiaztatzeko prozesua](workflow.md)
- [C++ oracle](oracle.md)
- [Migratu liburua](ledger.md)
- [errendimendua](performance.md)
- [Erregistroak berrikusi](issues.md)