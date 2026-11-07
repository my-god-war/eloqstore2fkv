# eloqstore2fkv

Sklad na overenie migrácie a kompatibility z eloqstore na fkv.

Hlavné úložisko fkv zostáva čistým Rustom a je zostavené nezávisle; tento repozitár uchováva referenčný implementačný oracle C++, obojsmerný bajtový korpus,
Migrujte účtovné knihy, historické záznamy auditu a overte CI.

## obsahu

- C++ oracle: prepojte vrstvu formátu eloqstore a vygenerujte stránku/index/pretečenie/manifest/
  komprimovať korpus.
- Obojsmerný korpus: C++ zápis → Rust čítanie, Rust zápis → C++ čítanie.
- `fkv_compat`: Test kompatibility na strane hrdze, spoliehajúci sa na formátovú prepravku skladu fkv.
- `fkv_ledger`: Skenujte eloqstore, zaznamenávajte knihu migrácie, generujte a overujte korpus.
- Porovnanie výkonu: Porovnajte základné línie fkv a C++ pri rovnakom zaťažení.

## Vchod

- [Proces overovania](workflow.md)
- [C++ oracle](oracle.md)
- [Migrovať účtovnú knihu](ledger.md)
- [výkon](performance.md)
- [Preskúmajte záznamy](issues.md)