# C++ oracle

`cpp/oracle` ប្រើអ្នកបង្កើត/ញែកពិតប្រាកដរបស់ eloqstore ដើម្បីបង្កើត corpus បៃ។

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ទទេ / តែមួយ / ច្រើន (កន្លែងដាក់) |
| store | ទទេ / តែមួយ / ច្រើន (កន្លែងដាក់) |

## ចំណុចប្រទាក់

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

លេខកូដចេញ 0 ត្រូវបានចាត់ទុកថាជោគជ័យ។ `verify` ត្រូវតែឆ្លងកាត់យ៉ាងច្បាស់ `kind` / `case`។

## បង្កើតភាពអាស្រ័យ

ធុងផ្ទុកលីនុចទាមទារ `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`។ Oracle នឹងប្រើកូដប្រភព abseil ដែលរក្សាទុកក្នុង eloqstore pin ដើម្បីបង្កើតបណ្ណាល័យឋិតិវន្ត។