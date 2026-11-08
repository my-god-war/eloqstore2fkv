# စိစစ်ရေးလုပ်ငန်းစဉ်

## သံချေးနှင့်လိုက်ဖက်မှုစမ်းသပ်ခြင်း။

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` သည် `../fkv` ဆွေမျိုးလမ်းကြောင်းများမှတဆင့် ဖော်မတ်သေတ္တာပေါ်တွင် မှီခိုသည်၊ ထည့်သွင်းသည့် ကော်ပိုရိတ်ကို ကုဒ်လုပ်သည်၊
ဘိုက်အလိုက် ဘိုက်ရွှေစံ ပြောဆိုချက်များ။

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle သည် TU ဖော်မတ်အလွှာကိုသာ စုစည်းပြီး io_uring / S3 / စာကြည့်တိုက်တစ်ခုလုံး တည်ဆောက်မှု မလိုအပ်ပါ။

## ရုပ်အလောင်း

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Drift ကိုစစ်ဆေးရန် CI သည် `fixtures/cpp`ကို ပြန်လည်ထုတ်ပေးပြီးနောက် `git diff --exit-code fixtures/cpp`ကို လုပ်ဆောင်ပါ။

## အခြေခံစာရင်း

`COMPAT_BASELINE` eloqstore commit နှင့် လိုက်ဖက်ညီသော အဆင့်များကို သော့ခတ်ထားသည်။ အဆင့်မြှင့်ထားသော အခြေခံလိုင်းအား ပြန်လည်စစ်ဆေးပြီး ပြန်လည်စစ်ဆေးရပါမည်။