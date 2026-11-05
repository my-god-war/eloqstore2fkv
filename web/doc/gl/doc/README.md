# eloqstore2fkv

Almacén de verificación de migración e compatibilidade de eloqstore a fkv.

O repositorio fkv principal segue sendo Rust puro e está construído de forma independente; este repositorio conserva o oráculo de implementación de referencia C++, corpus de bytes bidireccionais,
Migre libros de contabilidade, rexistros históricos de auditoría e verifique CI.

## contido

- Oracle C++: vincula a capa de formato eloqstore e xera páxina/índice/desbordamento/manifesto/
  comprimir o corpus.
- Corpus bidireccional: escritura C++ → lectura Rust, escritura Rust → lectura C++.
- `fkv_compat`: proba de compatibilidade do lado do ferruxe, baseándose na caixa de formato do almacén fkv.
- `fkv_ledger`: Analiza eloqstore, rexistra o libro de migración, xera e verifica corpus.
- Comparación de rendemento: compare as liñas de base fkv e C++ baixo a mesma carga de traballo.

## Entrada

- [Proceso de verificación](workflow.md)
- [C++ oracle](oracle.md)
- [Migrar libro](ledger.md)
- [rendemento](performance.md)
- [Revisar rexistros](issues.md)