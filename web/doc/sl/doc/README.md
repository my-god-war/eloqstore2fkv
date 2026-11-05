# eloqstore2fkv

Skladišče za selitev in preverjanje združljivosti iz eloqstore v fkv.

Glavno skladišče fkv ostaja čisti Rust in je zgrajeno neodvisno; ta repozitorij ohranja orakel referenčne implementacije C++, dvosmerni bajtni korpus,
Preselite glavne knjige, zgodovinske revizijske zapise in preverite KI.

## vsebino

- C++ oracle: povežite sloj formata eloqstore in ustvarite page/index/overflow/manifest/
  stisniti korpus.
- Dvosmerni korpus: C++ pisanje → Rust branje, Rust pisanje → C++ branje.
- `fkv_compat`: Test združljivosti na strani Rust, ki se zanaša na zaboj formata skladišča fkv.
- `fkv_ledger`: Skenirajte eloqstore, zabeležite migracijsko knjigo, ustvarite in preverite korpus.
- Primerjava zmogljivosti: primerjajte osnovne črte fkv in C++ pri enaki delovni obremenitvi.

## Vhod

- [Postopek preverjanja](workflow.md)
- [C++ oracle](oracle.md)
- [Preseli knjigo](ledger.md)
- [uspešnost](performance.md)
- [Pregled zapisov](issues.md)