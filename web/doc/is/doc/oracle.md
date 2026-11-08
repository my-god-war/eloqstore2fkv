# C++ oracle

`cpp/oracle` Notaðu raunverulegan smiðju / þáttara eloqstore til að búa til bætihluta.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | tómur / einn / margfaldur (staðgjafi) |
| store | tómur / einn / margfaldur (staðgjafi) |

## viðmót

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Útgöngukóði 0 telst vel heppnaður. `verify` verður að standast skýrt `kind` / `case`.

## Byggja upp ósjálfstæði

Linux gámar þurfa `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle mun nota frumkóðann sem geymdur er í eloqstore pinnanum til að byggja upp kyrrstætt bókasafn.