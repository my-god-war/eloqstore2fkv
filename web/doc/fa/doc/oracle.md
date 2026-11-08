# C++ oracle

`cpp/oracle` از سازنده / تجزیه کننده واقعی eloqstore برای تولید مجموعه بایت استفاده کنید.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | خالی / تک / چند (جایگزین) |
| store | خالی / تک / چند (جایگزین) |

## رابط کاربری

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

کد خروج 0 موفق در نظر گرفته می شود. `verify` باید به صراحت ارسال شود `kind` / `case`.

## وابستگی ایجاد کنید

کانتینرهای لینوکس به `cmake`،`g++ >= 13`،`libzstd-dev`،`libgoogle-glog-dev`، نیاز دارند،
`libgflags-dev`،`libjsoncpp-dev`،`liburing-dev`،`libcurl4-openssl-dev`،
`libboost-all-dev`. اوراکل از کد منبع abseil ذخیره شده در پین eloqstore برای ساخت یک کتابخانه استاتیک استفاده می کند.