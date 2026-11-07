# C++ oracle

`cpp/oracle` Norėdami generuoti baitų korpusą, naudokite tikrąjį eloqstore kūrėją / analizatorių.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tuščias / vienas / keli (vietos rezervavimo vieta) |
| store | tuščias / vienas / keli (vietos rezervavimo vieta) |

## sąsaja

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Išėjimo kodas 0 laikomas sėkmingu. `verify` turi būti aiškiai perduota `kind` / `case`.

## Sukurkite priklausomybes

„Linux“ sudėtiniams rodiniams reikia `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. „Oracle“ statinei bibliotekai sukurti naudos abseil šaltinio kodą, saugomą eloqstore kaištyje.