# Proces weryfikacji

## Testowanie kompatybilności z rdzą

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` opiera się na formacie `../fkv` poprzez ścieżki względne, dekoduje korpus wejściowy i nie
Twierdzenia złotego standardu bajt po bajcie.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle kompiluje tylko warstwę formatu TU i nie wymaga konstrukcji io_uring / S3 / całej biblioteki.

## ciało

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Wykonaj `git diff --exit-code fixtures/cpp` po ponownym wygenerowaniu CI `fixtures/cpp`, aby sprawdzić dryf.

## linia bazowa

`COMPAT_BASELINE` Blokuje poziomy zatwierdzania i zgodności eloqstore. Uaktualnioną linię bazową należy ponownie przeskanować i zweryfikować.