# Patvirtinimo procesas

## Suderinamumo su rūdimis bandymas

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` remiasi `../fkv` formato dėžute per santykinius kelius, dekoduoja įvesties korpusą ir atlieka
Auksinio standarto tvirtinimai baitas po baito.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

„Oracle“ kompiliuoja tik formato sluoksnį TU ir nereikalauja io_uring / S3 / visos bibliotekos kūrimo.

## korpusas

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Vykdykite `git diff --exit-code fixtures/cpp` po to, kai CI atsinaujins `fixtures/cpp`, kad patikrintumėte, ar nėra dreifo.

## pradinė linija

`COMPAT_BASELINE` Užrakina eloqstore įsipareigojimo ir suderinamumo lygius. Atnaujinta bazinė linija turi būti iš naujo nuskaityta ir patvirtinta.