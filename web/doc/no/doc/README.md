# eloqstore2fkv

Migrerings- og kompatibilitetsverifiseringslager fra eloqstore til fkv.

Hoveddepotet for fkv forblir rent Rust og bygges uavhengig; dette depotet beholder C++ referanseimplementeringsoraklet, toveis bytekorpus,
Migrer reskontro, historiske revisjonsposter og verifiser CI.

## innhold

- C++ oracle: koble eloqstore formatlag og generer side/indeks/overflyt/manifest/
  komprimere korpus.
- Toveis korpus: C++ skriv → Rust les, Rust skriv → C++ les.
- `fkv_compat`: Rustside-kompatibilitetstest, avhengig av formatkassen til fkv-lageret.
- `fkv_ledger`: Skann eloqstore, registrer migrasjonsreskontro, generer og verifiser korpus.
- Ytelsessammenligning: Sammenlign fkv og C++ grunnlinjer under samme arbeidsbelastning.

## Inngang

- [Verifiseringsprosess](workflow.md)
- [C++ oracle](oracle.md)
- [Overfør hovedbok](ledger.md)
- [ytelse](performance.md)
- [Gjennomgå poster](issues.md)