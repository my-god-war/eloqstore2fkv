# eloqstore2fkv

Migrációs és kompatibilitás-ellenőrző raktár az eloqstore-ból az fkv-be.

A fő fkv tároló tiszta Rust marad, és önállóan épül fel; ez a tároló megtartja a C++ referencia implementációs oracle-t, a kétirányú bájt korpuszt,
Áttelepítse a főkönyveket, a korábbi ellenőrzési rekordokat és ellenőrizze a CI-t.

## tartalom

- C++ oracle: kapcsolja össze az eloqstore formátumréteget, és hozza létre az oldalt/indexet/overflow/manifest/
  tömöríti a korpuszt.
- Kétirányú korpusz: C++ írás → Rust olvasás, Rust írás → C++ olvasás.
- `fkv_compat`: Rozsdaoldali kompatibilitási teszt, az fkv raktár formátumládájára támaszkodva.
- `fkv_ledger`: Az eloqstore vizsgálata, migrációs főkönyv rögzítése, korpusz létrehozása és ellenőrzése.
- Teljesítmény-összehasonlítás: Hasonlítsa össze az fkv és a C++ alapvonalakat azonos munkaterhelés mellett.

## Bejárat

- [Ellenőrzési folyamat](workflow.md)
- [C++ oracle](oracle.md)
- [Főkönyv migrálása](ledger.md)
- [teljesítmény](performance.md)
- [Feljegyzések áttekintése](issues.md)