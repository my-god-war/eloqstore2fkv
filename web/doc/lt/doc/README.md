# eloqstore2fkv

Perkėlimo ir suderinamumo tikrinimo sandėlis iš eloqstore į fkv.

Pagrindinė fkv saugykla išlieka gryna rūdys ir yra pastatyta savarankiškai; ši saugykla išlaiko C++ nuorodos įgyvendinimo orakulą, dvikryptį baitų korpusą,
Perkelkite knygas, istorinius audito įrašus ir patikrinkite CI.

## turinį

- C++ oracle: susiekite eloqstore formato sluoksnį ir sugeneruokite puslapį/index/overflow/manifest/
  suspausti korpusą.
- Dvikryptis korpusas: C++ rašymas → Rust skaitymas, Rūdžių rašymas → C++ skaitymas.
- `fkv_compat`: rūdžių pusės suderinamumo testas, pagrįstas fkv sandėlio formato dėžute.
- `fkv_ledger`: nuskaitykite eloqstore, įrašykite perkėlimo knygą, generuokite ir patikrinkite korpusą.
- Našumo palyginimas: palyginkite fkv ir C++ bazines linijas esant tam pačiam darbo krūviui.

## Įėjimas

- [Patvirtinimo procesas](workflow.md)
- [C++ oracle](oracle.md)
- [Perkelti knygą](ledger.md)
- [pasirodymas](performance.md)
- [Peržiūrėkite įrašus](issues.md)