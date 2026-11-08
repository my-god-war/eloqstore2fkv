# पडताळणी प्रक्रिया

## गंज सहत्वता चाचणी

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` सापेक्ष मार्गांद्वारे `../fkv` च्या फॉरमॅट क्रेटवर अवलंबून असते, इनपुट कॉर्पस डीकोड करते आणि करते
बाइट-बाय-बाइट गोल्ड मानक प्रतिपादन.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle फक्त TU हे फॉरमॅट लेयर संकलित करते आणि त्याला io_uring / S3 / संपूर्ण लायब्ररी बांधकामाची आवश्यकता नसते.

## कॉर्पस

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

ड्रिफ्ट तपासण्यासाठी CI रीजनरेट झाल्यानंतर `git diff --exit-code fixtures/cpp` कार्यान्वित करा.

## बेसलाइन

`COMPAT_BASELINE` eloqstore कमिट आणि सुसंगतता स्तर लॉक करते. श्रेणीसुधारित बेसलाइन पुन्हा स्कॅन करणे आणि पुन्हा सत्यापित करणे आवश्यक आहे.