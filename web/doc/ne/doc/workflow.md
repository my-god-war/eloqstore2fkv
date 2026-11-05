# प्रमाणीकरण प्रक्रिया

## रस्ट अनुकूलता परीक्षण

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` सापेक्ष मार्गहरू मार्फत `../fkv` को ढाँचा क्रेटमा निर्भर गर्दछ, इनपुट कोर्पस डिकोड गर्दछ र गर्छ
बाइट-द्वारा-बाइट सुन मानक दावीहरू।

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle ले TU ढाँचा तह मात्र कम्पाइल गर्छ र io_uring / S3 / सम्पूर्ण पुस्तकालय निर्माण आवश्यक पर्दैन।

## कोष

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

बहाव जाँच गर्न CI ले `fixtures/cpp` पुन: उत्पन्न गरेपछि `git diff --exit-code fixtures/cpp` कार्यान्वयन गर्नुहोस्।

## आधार रेखा

`COMPAT_BASELINE` eloqstore प्रतिबद्धता र अनुकूलता स्तरहरू लक गर्दछ। अपग्रेड गरिएको आधाररेखा पुन: स्क्यान गरी पुन: प्रमाणित गरिनुपर्छ।