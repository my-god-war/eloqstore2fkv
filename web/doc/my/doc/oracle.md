# C++ oracle

`cpp/oracle` byte corpus ထုတ်လုပ်ရန် eloqstore ၏ အစစ်အမှန် တည်ဆောက်သူ / parser ကို အသုံးပြုပါ။

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ဗလာ/တစ်ခုတည်း/အစုံ (နေရာယူသူ) |
| store | ဗလာ/တစ်ခုတည်း/အစုံ (နေရာယူသူ) |

## ကြားခံ

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 ထွက်ပေါက်ကုဒ်တစ်ခု အောင်မြင်သည်ဟု ယူဆပါသည်။ `verify` အတိအလင်း `kind` / `case`ကို ဖြတ်ကျော်ရပါမည်။

## မှီခိုမှုတည်ဆောက်ပါ။

Linux ကွန်တိန်နာများသည် `cmake`၊`g++ >= 13`၊`libzstd-dev`၊`libgoogle-glog-dev`၊
`libgflags-dev`၊`libjsoncpp-dev`၊`liburing-dev`၊`libcurl4-openssl-dev`၊
`libboost-all-dev`။ Oracle သည် static library တစ်ခုတည်ဆောက်ရန်အတွက် eloqstore pin တွင်သိမ်းဆည်းထားသော abseil source code ကိုအသုံးပြုပါမည်။