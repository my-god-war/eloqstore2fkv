# Ստուգման գործընթաց

## Ժանգի համատեղելիության փորձարկում

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` հենվում է `../fkv`-ի ձևաչափի վրա հարաբերական ուղիներով, վերծանում է մուտքային կորպուսը և անում
Բայթ առ բայթ ոսկու ստանդարտի պնդումներ.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle-ը կազմում է միայն TU ֆորմատի շերտը և չի պահանջում io_uring / S3 / ամբողջ գրադարանի կառուցում:

## կորպուս

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Կատարեք `git diff --exit-code fixtures/cpp` CI-ի վերականգնումից հետո `fixtures/cpp`՝ դրեյֆը ստուգելու համար:

## բազային

`COMPAT_BASELINE` Locks eloqstore պարտավորությունների և համատեղելիության մակարդակները: Թարմացված ելակետը պետք է վերասկանավորվի և վերստուգվի: