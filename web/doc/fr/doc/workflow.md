# Processus de vérification

## Tests de compatibilité avec la rouille

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` s'appuie sur la caisse de format de `../fkv` via des chemins relatifs, décode le corpus d'entrée et le fait
Assertions de référence octet par octet.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle compile uniquement la couche de format TU et ne nécessite pas la construction de io_uring / S3 / bibliothèque entière.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Exécutez `git diff --exit-code fixtures/cpp` après la régénération de CI `fixtures/cpp` pour vérifier la dérive.

## ligne de base

`COMPAT_BASELINE` Verrouille les niveaux de validation et de compatibilité d'eloqstore. La ligne de base mise à niveau doit être réanalysée et revérifiée.