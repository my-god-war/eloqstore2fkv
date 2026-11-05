# C++ oracle

`cpp/oracle` બાઇટ કોર્પસ જનરેટ કરવા માટે eloqstore ના વાસ્તવિક બિલ્ડર / પાર્સરનો ઉપયોગ કરો.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ખાલી / સિંગલ / બહુ (પ્લેસહોલ્ડર) |
| store | ખાલી / સિંગલ / બહુ (પ્લેસહોલ્ડર) |

## ઈન્ટરફેસ

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 નો એક્ઝિટ કોડ સફળ માનવામાં આવે છે. `verify` સ્પષ્ટપણે પસાર થવું આવશ્યક છે `kind` / `case`.

## નિર્ભરતા બનાવો

Linux કન્ટેનરને `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`ની જરૂર છે,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. ઓરેકલ સ્ટેટિક લાઇબ્રેરી બનાવવા માટે eloqstore પિનમાં સંગ્રહિત abseil સોર્સ કોડનો ઉપયોગ કરશે.