# C++ oracle

`cpp/oracle` बाइट कर्पस उत्पन्न गर्न eloqstore को वास्तविक निर्माणकर्ता / पार्सर प्रयोग गर्नुहोस्।

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | खाली / एकल / बहु (प्लेसहोल्डर) |
| store | खाली / एकल / बहु (प्लेसहोल्डर) |

## इन्टरफेस

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

० को एक्जिट कोड सफल मानिन्छ। `verify` स्पष्ट रूपमा पास हुनुपर्छ `kind` / `case`।

## निर्भरताहरू निर्माण गर्नुहोस्

लिनक्स कन्टेनरहरूलाई `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev` आवश्यक हुन्छ
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`। Oracle ले स्थिर पुस्तकालय निर्माण गर्न eloqstore पिनमा भण्डारण गरिएको abseil स्रोत कोड प्रयोग गर्नेछ।