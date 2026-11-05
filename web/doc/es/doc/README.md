# eloqstore2fkv

Almacén de verificación de migración y compatibilidad de eloqstore a fkv.

El repositorio principal de fkv sigue siendo Rust puro y se construye de forma independiente; este repositorio conserva el oráculo de implementación de referencia de C++, el corpus de bytes bidireccional,
Migre libros de contabilidad, registros de auditoría históricos y verifique CI.

## contenido

- Oracle C++: vincula la capa de formato eloqstore y genera página/índice/desbordamiento/manifest/
  comprimir el corpus.
- Corpus bidireccional: escritura C++ → lectura Rust, escritura Rust → lectura C++.
- `fkv_compat`: Prueba de compatibilidad con Rust, basándose en la caja de formato del almacén fkv.
- `fkv_ledger`: escanea eloqstore, registra el libro de migración, genera y verifica el corpus.
- Comparación de rendimiento: compare las líneas base de fkv y C++ bajo la misma carga de trabajo.

## Entrada

- [Proceso de verificación](workflow.md)
- [C++ oracle](oracle.md)
- [Migrar libro mayor](ledger.md)
- [actuación](performance.md)
- [Revisar registros](issues.md)