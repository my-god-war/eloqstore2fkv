# C++ oracle

`cpp/oracle`Tumia kijenzi/kichanganuzi halisi cha eloqstore ili kuzalisha ushirika wa kawaida.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tupu / moja / nyingi (kishika nafasi) |
| store | tupu / moja / nyingi (kishika nafasi) |

## kiolesura

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Msimbo wa kuondoka wa 0 unachukuliwa kuwa umefaulu. `verify` lazima ipitishwe kwa uwazi `kind` / `case`.

## Jenga utegemezi

Vyombo vya Linux vinahitaji `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle itatumia msimbo wa chanzo wa abseil uliohifadhiwa kwenye pini ya eloqstore kuunda maktaba tuli.