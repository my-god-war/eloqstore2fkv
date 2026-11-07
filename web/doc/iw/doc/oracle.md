# C++ oracle

`cpp/oracle` השתמש בבונה / מנתח האמיתי של eloqstore כדי ליצור קורפוס בתים.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ריק / יחיד / רב (מציין מקום) |
| store | ריק / יחיד / רב (מציין מקום) |

## מִמְשָׁק

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

קוד יציאה של 0 נחשב מוצלח. `verify` חייב לעבור במפורש `kind` / `case`.

## בנה תלות

מיכלי לינוקס דורשים `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. אורקל תשתמש בקוד המקור של הסנפלינג המאוחסן בפין eloqstore כדי לבנות ספרייה סטטית.