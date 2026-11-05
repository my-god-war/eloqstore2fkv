# Тексеру процесі

## Тотпен үйлесімділік сынағы

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` салыстырмалы жолдар арқылы `../fkv` пішім жәшігіне сүйенеді, кіріс корпусын декодтайды және жасайды
Байт-байт алтын стандартты бекітулер.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle тек TU форматтық қабатын құрастырады және io_uring / S3 / бүкіл кітапхана құрылысын қажет етпейді.

## корпус

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Дрейфті тексеру үшін CI қалпына келтіргеннен кейін `git diff --exit-code fixtures/cpp` орындаңыз.

## базалық

`COMPAT_BASELINE` eloqstore міндеттемесі мен үйлесімділік деңгейлерін құлыптайды. Жаңартылған негізгі сызық қайта сканерленіп, қайта тексерілуі керек.