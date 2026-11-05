# C++ oracle

`cpp/oracle` বাইট কর্পাস তৈরি করতে ইলোকস্টোরের আসল নির্মাতা / পার্সার ব্যবহার করুন৷

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | খালি / একক / বহু (স্থানধারক) |
| store | খালি / একক / বহু (স্থানধারক) |

## ইন্টারফেস

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 এর একটি প্রস্থান কোড সফল বলে বিবেচিত হয়। `verify` স্পষ্টভাবে পাস করতে হবে `kind` / `case`৷

## নির্ভরতা তৈরি করুন

লিনাক্স কন্টেইনারগুলির প্রয়োজন `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`। ওরাকল একটি স্ট্যাটিক লাইব্রেরি তৈরি করতে eloqstore পিনে সংরক্ষিত abseil সোর্স কোড ব্যবহার করবে।