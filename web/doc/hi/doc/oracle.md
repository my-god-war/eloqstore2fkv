# C++ oracle

`cpp/oracle` बाइट कॉर्पस उत्पन्न करने के लिए eloqstore के वास्तविक बिल्डर/पार्सर का उपयोग करें।

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

## इंटरफ़ेस

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 का निकास कोड सफल माना जाता है। `verify` को स्पष्ट रूप से `kind` / `case` पारित किया जाना चाहिए।

## निर्भरताएँ बनाएँ

लिनक्स कंटेनरों को `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`, की आवश्यकता होती है
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
<बी आईडी='1'>. Oracle एक स्थिर लाइब्रेरी बनाने के लिए eloqstore पिन में संग्रहीत abseil स्रोत कोड का उपयोग करेगा।