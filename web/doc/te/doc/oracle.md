# C++ oracle

`cpp/oracle` బైట్ కార్పస్‌ను రూపొందించడానికి eloqstore యొక్క నిజమైన బిల్డర్ / పార్సర్‌ని ఉపయోగించండి.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ఖాళీ / సింగిల్ / బహుళ (ప్లేస్‌హోల్డర్) |
| store | ఖాళీ / సింగిల్ / బహుళ (ప్లేస్‌హోల్డర్) |

## ఇంటర్ఫేస్

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 యొక్క నిష్క్రమణ కోడ్ విజయవంతంగా పరిగణించబడుతుంది. `verify`ని ఖచ్చితంగా `kind` / `case` పాస్ చేయాలి.

## డిపెండెన్సీలను నిర్మించండి

Linux కంటైనర్‌లకు `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`, అవసరం
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. ఒరాకిల్ స్టాటిక్ లైబ్రరీని నిర్మించడానికి eloqstore పిన్‌లో నిల్వ చేసిన abseil సోర్స్ కోడ్‌ను ఉపయోగిస్తుంది.