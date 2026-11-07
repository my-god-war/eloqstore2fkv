# عملية التحقق

## اختبار توافق الصدأ

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` يعتمد على صندوق التنسيق `../fkv` من خلال المسارات النسبية، ويفك تشفير مجموعة الإدخال ويفعل ذلك
التأكيدات القياسية الذهبية بايت بايت.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

تقوم Oracle بتجميع طبقة التنسيق TU فقط ولا تتطلب إنشاء مكتبة io_uring / S3 / بأكملها.

## مجموعة

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

نفذ `git diff --exit-code fixtures/cpp` بعد إعادة إنشاء CI `fixtures/cpp` للتحقق من الانجراف.

## خط الأساس

`COMPAT_BASELINE` يقفل مستويات الالتزام والتوافق في eloqstore. يجب إعادة فحص خط الأساس الذي تمت ترقيته وإعادة التحقق منه.