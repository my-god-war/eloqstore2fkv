# C++ oracle

`cpp/oracle` byte corpus උත්පාදනය කිරීමට eloqstore හි සැබෑ තනන්නා / විග්‍රහකය භාවිතා කරන්න.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | හිස් / තනි / බහු (තැන් දරන්නා) |
| store | හිස් / තනි / බහු (තැන් දරන්නා) |

## අතුරු මුහුණත

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 හි පිටවීමේ කේතය සාර්ථක ලෙස සැලකේ. `verify` පැහැදිලිවම සමත් විය යුතුය `kind` / `case`.

## පරායත්තතා ගොඩ නගන්න

Linux බහාලුම් සඳහා `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`, අවශ්‍ය වේ
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle විසින් ස්ථිතික පුස්තකාලයක් තැනීම සඳහා eloqstore pin හි ගබඩා කර ඇති abseil මූල කේතය භාවිතා කරනු ඇත.