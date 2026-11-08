# eloqstore2fkv

Maħżen tal-verifika tal-migrazzjoni u l-kompatibilità minn eloqstore għal fkv.

Ir-repożitorju tal-fkv ewlieni jibqa' Rust pur u jinbena b'mod indipendenti; dan ir-repożitorju jżomm l-oraklu ta' implimentazzjoni ta' referenza C++, corpus ta' byte bidirezzjonali,
Imigra r-reġistru, ir-rekords storiċi tal-awditjar u vverifika CI.

## kontenut

- Oracle C++: għaqqad is-saff tal-format eloqstore u tiġġenera paġna/indiċi/overflow/manifest/
  kompress corpus.
- Corpus bidirezzjonali: C++ kitba → Rust aqra, Rust kitba → C++ aqra.
- `fkv_compat`: Test tal-kompatibilità tan-naħa tas-sadid, billi bbażat ruħha fuq iċ-ċestun tal-format tal-maħżen fkv.
- `fkv_ledger`: Skennja eloqstore, irreġistra r-reġistru tal-migrazzjoni, iġġenera u vverifika l-korpus.
- Tqabbil tal-prestazzjoni: Qabbel il-linji bażi fkv u C++ taħt l-istess ammont ta 'xogħol.

## Dħul

- [Proċess ta' verifika](workflow.md)
- [C++ oracle](oracle.md)
- [Emigra l-ktieb](ledger.md)
- [prestazzjoni](performance.md)
- [Ir-reġistri tar-reviżjoni](issues.md)