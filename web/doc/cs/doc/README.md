# eloqstore2fkv

Sklad pro migraci a ověřování kompatibility z eloqstore na fkv.

Hlavní úložiště fkv zůstává čistým Rustem a je sestaveno nezávisle; toto úložiště uchovává referenční implementační oracle C++, obousměrný byte korpus,
Migrujte účetní knihy, historické záznamy auditu a ověřte CI.

## obsah

- C++ oracle: propojte vrstvu formátu eloqstore a vygenerujte stránku/index/přetečení/manifest/
  komprimovat korpus.
- Obousměrný korpus: C++ zápis → Rust čtení, Rust zápis → C++ čtení.
- `fkv_compat`: Test kompatibility na straně rzi se spoléhá na formátovou bednu skladu fkv.
- `fkv_ledger`: Skenujte eloqstore, zaznamenejte knihu migrace, vygenerujte a ověřte korpus.
- Porovnání výkonu: Porovnejte základní linie fkv a C++ při stejném pracovním zatížení.

## Vjezd

- [Ověřovací proces](workflow.md)
- [C++ oracle](oracle.md)
- [Přenést účetní knihu](ledger.md)
- [výkon](performance.md)
- [Zkontrolujte záznamy](issues.md)