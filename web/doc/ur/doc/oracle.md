# C++ oracle

`cpp/oracle` بائٹ کارپس بنانے کے لیے eloqstore کے اصلی بلڈر / پارسر کا استعمال کریں۔

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | خالی / واحد / کثیر (پلیس ہولڈر) |
| store | خالی / واحد / کثیر (پلیس ہولڈر) |

## انٹرفیس

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 کا ایگزٹ کوڈ کامیاب سمجھا جاتا ہے۔ `verify` کو واضح طور پر پاس کیا جانا چاہیے `kind` / `case`۔

## انحصار بنائیں

لینکس کنٹینرز کو `cmake`،`g++ >= 13`،`libzstd-dev`،`libgoogle-glog-dev` کی ضرورت ہوتی ہے،
`libgflags-dev`،`libjsoncpp-dev`،`liburing-dev`،`libcurl4-openssl-dev`،
`libboost-all-dev`۔ اوریکل ایک جامد لائبریری بنانے کے لیے eloqstore پن میں محفوظ abseil سورس کوڈ کا استعمال کرے گا۔