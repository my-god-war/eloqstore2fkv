# eloqstore2fkv

Migratioun an Onbedenklechkeet Verifizéierungslager vun eloqstore op fkv.

Den Haaptfkv Repository bleift pure Rust a gëtt onofhängeg gebaut; Dëse Repository behält d'C++ Referenz Implementatioun Oracle, bidirektional Byte Corpus,
Migréieren Ledger, historesch Audit records an z'iwwerpréiwen CI.

## Inhalt

- C++ Oracle: Link eloqstore Format Layer a generéiert Säit / Index / Iwwerfloss / Manifest /
  kompriméieren Corpus.
- Bidirektional Corpus: C++ schreiwen → Rust liesen, Rust schreiwen → C++ liesen.
- `fkv_compat`: Rust-Säit Kompatibilitéitstest, vertrauen op d'Formatkrat vum fkv Lagerhaus.
- `fkv_ledger`: Scan eloqstore, Rekord Migratioun Ledger, generéieren an z'iwwerpréiwen Corpus.
- Leeschtung Verglach: Vergläichen fkv an C ++ baselines ënner der selwechter Aarbechtslaascht.

## Entrée

- [Verifikatiounsprozess](workflow.md)
- [C++ oracle](oracle.md)
- [Migréieren Ledger](ledger.md)
- [Leeschtung](performance.md)
- [Iwwerpréiwen records](issues.md)