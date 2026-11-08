# စာရင်းဇယားကို ရွှေ့ပြောင်းပါ။

`tool/fkv_ledger` eloqstore ကိုစကင်န်ဖတ်ရန်၊ မြေပုံနှင့် C++ သင်္ကေတများနှင့် fkv ကိုအတည်ပြုရန် သစ်ပင်ထိန်းကိုသုံးပါ
အထောက်အထားကို motedb တွင်ရေးသားထားသည်။

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

လယ်ဂျာဒေတာဘေ့စ်သည် `.data/motedb.mote` တွင်ရှိပြီး ဒေတာဘေ့စ်တွင် သိမ်းဆည်းထားခြင်းမရှိပါ။

`verified` သက်သေအထောက်အထားရှိရမည်။ `doc/progress.md`ကို `report` မှ ထုတ်လုပ်သည်။