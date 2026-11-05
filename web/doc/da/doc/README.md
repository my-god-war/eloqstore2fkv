# eloqstore2fkv

Migration og kompatibilitet verifikation lager fra eloqstore til fkv.

Hoveddepotet for fkv forbliver rent Rust og er bygget selvstændigt; dette lager beholder C++-referenceimplementeringsoraklet, tovejs bytekorpus,
Migrer regnskaber, historiske revisionsposter og verificere CI.

## tilfreds

- C++ oracle: link eloqstore formatlag og generer side/indeks/overløb/manifest/
  komprimere korpus.
- Tovejskorpus: C++ skriv → Rust læs, Rust skriv → C++ læs.
- `fkv_compat`: Rust-side-kompatibilitetstest, afhængig af formatkassen på fkv-lageret.
- `fkv_ledger`: Scan eloqstore, optag migrationsreskontro, generer og verificere korpus.
- Præstationssammenligning: Sammenlign fkv og C++ baselines under samme arbejdsbelastning.

## Indgang

- [Verifikationsproces](workflow.md)
- [C++ oracle](oracle.md)
- [Overfør hovedbog](ledger.md)
- [præstation](performance.md)
- [Gennemgå optegnelser](issues.md)