# תהליך אימות

## בדיקת תאימות חלודה

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` מסתמך על ארגז הפורמט של `../fkv` דרך נתיבים יחסיים, מפענח את קורפוס הקלט ועושה
הצהרות תקן זהב בייט אחר בייט.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

אורקל מרכיבה רק את שכבת הפורמט TU ואינה דורשת io_uring / S3 / בניית ספרייה כולה.

## קורפוס

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

בצע את `git diff --exit-code fixtures/cpp` לאחר ש-CI ייצור מחדש את `fixtures/cpp` כדי לבדוק סחיפה.

## קו בסיס

`COMPAT_BASELINE` נועלת את רמות ההתחייבות והתאימות של eloqstore. יש לסרוק מחדש את קו הבסיס המשודרג ולאמת אותו מחדש.