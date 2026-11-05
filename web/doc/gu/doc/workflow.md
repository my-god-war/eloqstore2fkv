# ચકાસણી પ્રક્રિયા

## રસ્ટ સુસંગતતા પરીક્ષણ

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` સંબંધિત પાથ દ્વારા `../fkv` ના ફોર્મેટ ક્રેટ પર આધાર રાખે છે, ઇનપુટ કોર્પસને ડીકોડ કરે છે અને કરે છે
બાઇટ-બાય-બાઇટ ગોલ્ડ સ્ટાન્ડર્ડ નિવેદનો.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

ઓરેકલ માત્ર ફોર્મેટ લેયર TU કમ્પાઈલ કરે છે અને તેને io_uring / S3 / સમગ્ર લાઈબ્રેરી બાંધકામની જરૂર નથી.

## કોર્પસ

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

ડ્રિફ્ટ તપાસવા માટે CI પુનઃજનરેટ થાય તે પછી `git diff --exit-code fixtures/cpp` એક્ઝિક્યુટ કરો.

## આધારરેખા

`COMPAT_BASELINE` eloqstore પ્રતિબદ્ધતા અને સુસંગતતા સ્તરોને લૉક કરે છે. અપગ્રેડ કરેલ બેઝલાઇન ફરીથી સ્કેન અને પુનઃ ચકાસાયેલ હોવી આવશ્યક છે.