# eloqstore2fkv

Paglalin ug pagkaangay nga bodega sa pag-verify gikan sa eloqstore hangtod sa fkv.

Ang nag-unang fkv repository nagpabilin nga puro Rust ug gitukod nga independente; kini nga repository nagpabilin sa C++ reference nga pagpatuman sa orakulo, bidirectional byte corpus,
Pagbalhin sa mga ledger, mga rekord sa pag-audit sa kasaysayan ug pag-verify sa CI.

## sulod

- C++ oracle: i-link ang eloqstore format layer ug paghimo og page/index/overflow/manifest/
  compress corpus.
- Bidirectional corpus: C++ pagsulat → Rust pagbasa, Rust pagsulat → C++ basaha.
- `fkv_compat`: Rust-side compatibility test, nagsalig sa format nga crate sa fkv bodega.
- `fkv_ledger`: Pag-scan sa eloqstore, pagrekord sa ledger sa paglalin, paghimo ug pag-verify sa corpus.
- Pagtandi sa performance: Itandi ang fkv ug C++ baseline ubos sa samang workload.

## Entrance

- [Proseso sa pag-verify](workflow.md)
- [C++ oracle](oracle.md)
- [Pagbalhin sa ledger](ledger.md)
- [pasundayag](performance.md)
- [Repasuha ang mga rekord](issues.md)