# გადამოწმების პროცესი

## ჟანგის თავსებადობის ტესტირება

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` ეყრდნობა `../fkv`-ის ფორმატის კრატს ფარდობითი გზების მეშვეობით, დეკოდირებს შეყვანის კორპუსს და აკეთებს
ბაიტი-ბაიტი ოქროს სტანდარტის მტკიცებულებები.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle ადგენს მხოლოდ ფორმატის ფენას TU და არ საჭიროებს io_uring / S3 / ბიბლიოთეკის მთლიან კონსტრუქციას.

## კორპუსი

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

შეასრულეთ `git diff --exit-code fixtures/cpp` CI რეგენერაციის შემდეგ `fixtures/cpp` დრიფტის შესამოწმებლად.

## საბაზისო

`COMPAT_BASELINE` Locks eloqstore commit და თავსებადობის დონეები. განახლებული საბაზისო ხაზი ხელახლა უნდა იყოს დასკანირებული და გადამოწმებული.