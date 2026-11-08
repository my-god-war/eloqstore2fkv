# C++ oracle

`cpp/oracle` ବାଇଟ୍ କର୍ପସ୍ ସୃଷ୍ଟି କରିବାକୁ ଏଲୋକ୍ଷ୍ଟୋରର ପ୍ରକୃତ ବିଲଡର୍ / ପାର୍ସର୍ ବ୍ୟବହାର କରନ୍ତୁ |

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ଖାଲି / ଏକକ / ମଲ୍ଟି (ସ୍ଥାନଧାରୀ) |
| store | ଖାଲି / ଏକକ / ମଲ୍ଟି (ସ୍ଥାନଧାରୀ) |

## ଇଣ୍ଟରଫେସ୍

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 ର ଏକ ପ୍ରସ୍ଥାନ କୋଡ୍ ସଫଳ ବୋଲି ବିବେଚନା କରାଯାଏ | `verify` ସ୍ପଷ୍ଟ ଭାବରେ `kind` / `case` ପାସ୍ ହେବା ଜରୁରୀ |

## ନିର୍ଭରଶୀଳତା ଗଠନ କରନ୍ତୁ |

ଲିନକ୍ସ ପାତ୍ରଗୁଡିକ `cmake`, `g++ >= 13`, `libzstd-dev`, `libgoogle-glog-dev`,
`libgflags-dev`, `libjsoncpp-dev`, `liburing-dev`, `libcurl4-openssl-dev`,
`libboost-all-dev` | ଏକ ଷ୍ଟାଟିକ୍ ଲାଇବ୍ରେରୀ ନିର୍ମାଣ ପାଇଁ ଓରାକଲ୍ ଏଲୋକ୍ଷ୍ଟୋର ପିନ୍ରେ ଗଚ୍ଛିତ ଅବସେଲ୍ ଉତ୍ସ କୋଡ୍ ବ୍ୟବହାର କରିବ |