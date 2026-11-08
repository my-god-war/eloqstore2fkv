# eloqstore2fkv

Warehouse de migração e verificação de compatibilidade de eloqstore para fkv.

O repositório principal do fkv permanece puro Rust e é construído de forma independente; este repositório mantém o oráculo de implementação de referência C++, corpus de bytes bidirecionais,
Migre livros contábeis, registros históricos de auditoria e verifique CI.

## contente

- Oracle C++: vincula a camada de formato eloqstore e gera página/índice/overflow/manifest/
  comprimir corpus.
- Corpus bidirecional: gravação C++ → leitura Rust, gravação Rust → leitura C++.
- `fkv_compat`: Teste de compatibilidade do lado da ferrugem, contando com a caixa de formato do warehouse fkv.
- `fkv_ledger`: Digitalize o eloqstore, registre o registro de migração, gere e verifique o corpus.
- Comparação de desempenho: compare as linhas de base fkv e C++ na mesma carga de trabalho.

## Entrada

- [Processo de verificação](workflow.md)
- [C++ oracle](oracle.md)
- [Migrar razão](ledger.md)
- [desempenho](performance.md)
- [Revise os registros](issues.md)