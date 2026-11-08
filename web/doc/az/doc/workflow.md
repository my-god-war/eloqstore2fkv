# Doğrulama prosesi

## Pas uyğunluğu testi

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` nisbi yollar vasitəsilə `../fkv` format qutusuna əsaslanır, giriş korpusunu deşifrə edir və
Bayt-bayt qızıl standart təsdiqlər.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle yalnız TU format qatını tərtib edir və io_uring / S3 / bütün kitabxananın qurulmasını tələb etmir.

## korpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Drift olub-olmadığını yoxlamaq üçün CI regenerasiya etdikdən sonra `git diff --exit-code fixtures/cpp` yerinə yetirin.

## əsas

`COMPAT_BASELINE` Eloqstore öhdəliyini və uyğunluq səviyyələrini kilidləyir. Təkmilləşdirilmiş baza yenidən skan edilməli və yenidən yoxlanılmalıdır.