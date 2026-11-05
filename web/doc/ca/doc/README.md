# eloqstore2fkv

Magatzem de verificació de migració i compatibilitat d'eloqstore a fkv.

El repositori fkv principal continua sent pur Rust i es construeix de manera independent; aquest repositori conserva l'oracle d'implementació de referència de C++, corpus de bytes bidireccionals,
Migreu llibres majors, registres històrics d'auditoria i verifiqueu CI.

## contingut

- Oracle C++: enllaça la capa de format eloqstore i genera pàgina/índex/desbordament/manifest/
  comprimir el corpus.
- Corpus bidireccional: C++ escriptura → Rust read, Rust write → C++ read.
- `fkv_compat`: prova de compatibilitat del costat de l'òxid, basant-se en la caixa de format del magatzem fkv.
- `fkv_ledger`: escaneja eloqstore, registra el llibre de migració, genera i verifica el corpus.
- Comparació de rendiment: compareu les línies de base fkv i C++ amb la mateixa càrrega de treball.

## Entrada

- [Procés de verificació](workflow.md)
- [C++ oracle](oracle.md)
- [Migració del llibre major](ledger.md)
- [rendiment](performance.md)
- [Revisar registres](issues.md)