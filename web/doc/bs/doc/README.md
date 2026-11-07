# eloqstore2fkv

Skladište za migraciju i verifikaciju kompatibilnosti sa eloqstore na fkv.

Glavno fkv spremište ostaje čisto Rust i izgrađeno je nezavisno; ovo spremište zadržava proročište implementacije referentne C++, dvosmjerni bajt korpus,
Migrirajte glavne knjige, historijske revizijske zapise i provjerite CI.

## sadržaj

- C++ oracle: povežite sloj formata eloqstore i generirajte stranicu/index/overflow/manifest/
  komprimirati korpus.
- Dvosmjerni korpus: C++ pisanje → Rust čitanje, Rust pisanje → C++ čitanje.
- `fkv_compat`: Test kompatibilnosti na strani Rust-a, oslanjajući se na kutiju formata fkv skladišta.
- `fkv_ledger`: Skenirajte eloqstore, snimite knjigu migracije, generirajte i provjerite korpus.
- Poređenje performansi: Uporedite fkv i C++ osnovne linije pod istim radnim opterećenjem.

## Ulaz

- [Proces verifikacije](workflow.md)
- [C++ oracle](oracle.md)
- [Migracija knjige](ledger.md)
- [performanse](performance.md)
- [Pregledajte zapise](issues.md)