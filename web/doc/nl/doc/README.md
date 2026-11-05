# eloqstore2fkv

Migratie- en compatibiliteitsverificatiemagazijn van eloqstore naar fkv.

De belangrijkste fkv-repository blijft puur Rust en wordt onafhankelijk gebouwd; deze repository behoudt het C++ referentie-implementatie-orakel, bidirectioneel bytecorpus,
Migreer grootboeken en historische auditgegevens en verifieer CI.

## inhoud

- C++ orakel: koppel de eloqstore-formaatlaag en genereer pagina/index/overflow/manifest/
  corpus comprimeren.
- Bidirectioneel corpus: C++ schrijven → Rust lezen, Rust schrijven → C++ lezen.
- `fkv_compat`: Rust-side compatibiliteitstest, gebaseerd op de formatkrat van het fkv-magazijn.
- `fkv_ledger`: eloqstore scannen, migratiegrootboek registreren, corpus genereren en verifiëren.
- Prestatievergelijking: vergelijk fkv- en C++-basislijnen onder dezelfde werklast.

## Ingang

- [Verificatieproces](workflow.md)
- [C++ oracle](oracle.md)
- [Grootboek migreren](ledger.md)
- [prestatie](performance.md)
- [Gegevens bekijken](issues.md)