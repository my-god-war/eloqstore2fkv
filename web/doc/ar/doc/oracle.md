# C++ oracle

`cpp/oracle` استخدم المُنشئ/المحلل الحقيقي لـ eloqstore لإنشاء مجموعة بايت.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | فارغ / فردي / متعدد (عنصر نائب) |
| store | فارغ / فردي / متعدد (عنصر نائب) |

## واجهة

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

يعتبر رمز الخروج 0 ناجحًا. يجب تمرير `verify` بشكل صريح `kind` / `case`.

## بناء التبعيات

تتطلب حاويات Linux `cmake`، `g++ >= 13`، `libzstd-dev`، `libgoogle-glog-dev`،
`libgflags-dev`، `libjsoncpp-dev`، `liburing-dev`، `libcurl4-openssl-dev`،
<ب معرف = "1">. ستستخدم Oracle كود مصدر abseil المخزن في دبوس eloqstore لإنشاء مكتبة ثابتة.