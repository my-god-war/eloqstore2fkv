# C++ oracle

`cpp/oracle` Օգտագործեք eloqstore-ի իրական շինարարը/վերլուծիչը՝ բայթերի կորպուս ստեղծելու համար:

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | դատարկ / միայնակ / բազմակի (տեղապահ) |
| store | դատարկ / միայնակ / բազմակի (տեղապահ) |

## ինտերֆեյս

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 ելքի կոդը համարվում է հաջողված: `verify`-ը պետք է հստակորեն փոխանցվի `kind` / `case`:

## Կառուցեք կախվածություն

Linux կոնտեյներները պահանջում են `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`: Oracle-ը կօգտագործի abseil սկզբնական կոդը, որը պահվում է eloqstore փինում՝ ստատիկ գրադարան ստեղծելու համար: