# eloqstore2fkv

Skladište za migraciju i provjeru kompatibilnosti s eloqstore na fkv.

Glavno fkv spremište ostaje čisti Rust i gradi se neovisno; ovo spremište zadržava C++ referentnu implementaciju proročanstva, dvosmjerni korpus bajtova,
Migrirajte knjige, povijesne revizijske zapise i provjerite CI.

## sadržaj

- C++ oracle: povežite sloj formata eloqstore i generirajte stranicu/indeks/overflow/manifest/
  stisnuti korpus.
- Dvosmjerni korpus: C++ pisanje → Rust čitanje, Rust pisanje → C++ čitanje.
- `fkv_compat`: Rust-side test kompatibilnosti, oslanjajući se na sanduk formata fkv skladišta.
- `fkv_ledger`: Skenirajte eloqstore, zabilježite migracijsku knjigu, generirajte i provjerite korpus.
- Usporedba performansi: Usporedite fkv i C++ osnovne linije pod istim radnim opterećenjem.

## Ulaz

- [Proces provjere](workflow.md)
- [C++ oracle](oracle.md)
- [Migracija knjige](ledger.md)
- [performanse](performance.md)
- [Pregledajte zapise](issues.md)