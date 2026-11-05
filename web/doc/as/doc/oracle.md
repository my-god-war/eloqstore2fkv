# C++ oracle

`cpp/oracle` বাইট কৰ্পাছ সৃষ্টি কৰিবলে eloqstore ৰ প্ৰকৃত বিল্ডাৰ / পাৰ্চাৰ ব্যৱহাৰ কৰক।

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | খালী / একক / বহু (প্লেচহোল্ডাৰ) |
| store | খালী / একক / বহু (প্লেচহোল্ডাৰ) |

## আন্তঃপৃষ্ঠ

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 ৰ এটা প্ৰস্থান ক'ড সফল বুলি গণ্য কৰা হয়। `verify` স্পষ্টভাৱে `kind` / `case` পাছ কৰিব লাগিব।

## নিৰ্ভৰশীলতা গঢ়ি তোলক

লিনাক্স ধাৰকসমূহৰ বাবে `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle এ eloqstore পিনত সংৰক্ষণ কৰা abseil উৎস ক'ড ব্যৱহাৰ কৰিব এটা স্থিতিশীল লাইব্ৰেৰী নিৰ্মাণ কৰিবলে।