# eloqstore2fkv

Magazzino di verifica della migrazione e della compatibilità da eloqstore a fkv.

Il repository principale di fkv rimane puro Rust ed è costruito in modo indipendente; questo repository conserva l'oracolo di implementazione di riferimento C++, il corpus di byte bidirezionale,
Migrazione di registri, registri storici di audit e verifica dell'IC.

## contenuto

- Oracolo C++: collega il livello di formato eloqstore e genera pagina/indice/overflow/manifest/
  comprimere il corpo.
- Corpus bidirezionale: scrittura C++ → lettura Rust, scrittura Rust → lettura C++.
- `fkv_compat`: test di compatibilità lato ruggine, basato sul formato cassa del magazzino fkv.
- `fkv_ledger`: scansiona eloqstore, registra il registro delle migrazioni, genera e verifica il corpus.
- Confronto delle prestazioni: confronta le linee di base fkv e C++ con lo stesso carico di lavoro.

## Entrata

- [Processo di verifica](workflow.md)
- [C++ oracle](oracle.md)
- [Migrare il registro](ledger.md)
- [prestazione](performance.md)
- [Esaminare i record](issues.md)