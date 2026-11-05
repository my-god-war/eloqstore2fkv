# eloqstore2fkv

Migrerings- och kompatibilitetsverifieringslager från eloqstore till fkv.

Huvudförvaret för fkv förblir rent Rust och är byggt självständigt; detta arkiv behåller C++-referensimplementerings-oraklet, dubbelriktad byte-korpus,
Migrera reskontra, historiska revisionsposter och verifiera CI.

## innehåll

- C++ oracle: länka eloqstore formatlager och generera sida/index/overflow/manifest/
  komprimera korpus.
- Dubbelriktad korpus: C++ skriv → Rust läs, Rust skriv → C++ läs.
- `fkv_compat`: Kompatibilitetstest på rostsidan, beroende på formatlådan för fkv-lagret.
- `fkv_ledger`: Skanna eloqstore, registrera migreringsreskontra, generera och verifiera korpus.
- Prestandajämförelse: Jämför fkv och C++ baslinjer under samma arbetsbelastning.

## Ingång

- [Verifieringsprocess](workflow.md)
- [C++ oracle](oracle.md)
- [Migrera huvudbok](ledger.md)
- [prestanda](performance.md)
- [Granska poster](issues.md)