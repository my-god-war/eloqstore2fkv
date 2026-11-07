# सत्यापन प्रक्रिया

## जंग अनुकूलता परीक्षण

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` सापेक्ष पथों के माध्यम से `../fkv` के प्रारूप क्रेट पर निर्भर करता है, इनपुट कॉर्पस को डीकोड करता है और करता है
बाइट-बाइट सोने के मानक दावे।

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle केवल प्रारूप परत TU को संकलित करता है और उसे io_uring / S3 / संपूर्ण लाइब्रेरी निर्माण की आवश्यकता नहीं होती है।

## कोर्पस

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

बहाव की जांच करने के लिए सीआई के पुन: उत्पन्न होने के बाद `git diff --exit-code fixtures/cpp` निष्पादित करें।

## आधारभूत

`COMPAT_BASELINE` eloqstore प्रतिबद्धता और संगतता स्तर को लॉक करता है। उन्नत बेसलाइन को पुनः स्कैन और पुनः सत्यापित किया जाना चाहिए।