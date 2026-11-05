# C++ oracle

`cpp/oracle` استعمال ڪريو eloqstore جي حقيقي بلڊر / پارسر بائيٽ ڪارپس پيدا ڪرڻ لاءِ.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | خالي / اڪيلو / گھڻائي (جڳھ ھولڊر) |
| store | خالي / اڪيلو / گھڻائي (جڳھ ھولڊر) |

## انٽرفيس

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 جو نڪرڻ وارو ڪوڊ ڪامياب سمجهيو ويندو آهي. `verify` واضح طور تي منظور ڪيو وڃي `kind` / `case`.

## انحصار پيدا ڪريو

لينڪس ڪنٽينرز جي ضرورت آهي `cmake`،`g++ >= 13`،`libzstd-dev`،`libgoogle-glog-dev`،
`libgflags-dev`،`libjsoncpp-dev`،`liburing-dev`،`libcurl4-openssl-dev`،
`libboost-all-dev`. Oracle استعمال ڪندو abseil سورس ڪوڊ محفوظ ٿيل eloqstore پن ۾ هڪ جامد لائبريري ٺاهڻ لاءِ.