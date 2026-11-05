# C++ oracle

`cpp/oracle` बाइट कॉर्पस व्युत्पन्न करण्यासाठी eloqstore चा वास्तविक बिल्डर/पार्सर वापरा.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | रिक्त / सिंगल / मल्टी (प्लेसहोल्डर) |
| store | रिक्त / सिंगल / मल्टी (प्लेसहोल्डर) |

## इंटरफेस

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 चा एक्झिट कोड यशस्वी मानला जातो. `verify` स्पष्टपणे पास करणे आवश्यक आहे `kind` / `case`.

## अवलंबित्व तयार करा

लिनक्स कंटेनरला `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev` आवश्यक आहे
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle स्टॅटिक लायब्ररी तयार करण्यासाठी eloqstore पिनमध्ये संग्रहित abseil सोर्स कोड वापरेल.