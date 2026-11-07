# eloqstore2fkv

Warws gwirio mudo a chydnawsedd o eloqstore i fkv.

Mae prif ystorfa fkv yn parhau i fod yn Rust pur ac wedi ei hadeiladu yn annibynol ; mae'r ystorfa hon yn cadw'r oracl gweithredu cyfeirnod C++, corpws beit deugyfeiriadol,
Mudo cyfriflyfrau, cofnodion archwilio hanesyddol a gwirio CI.

## cynnwys

- C ++ oracl: cysylltu haen fformat eloqstore a chynhyrchu tudalen / mynegai / gorlif / maniffest /
  cywasgu corpws.
- Corpws deugyfeiriadol: C++ ysgrifennu → Rhwd yn darllen, rhwd ysgrifennu → C++ darllen.
- `fkv_compat`: Prawf cydweddoldeb ochr rhwd, yn dibynnu ar grât fformat y warws fkv.
- `fkv_ledger`: Sganio eloqstore, cofnodi cyfriflyfr mudo, cynhyrchu a gwirio corpws.
- Cymharu perfformiad: Cymharwch linellau sylfaen fkv a C++ o dan yr un llwyth gwaith.

## Mynedfa

- [Proses ddilysu](workflow.md)
- [C++ oracle](oracle.md)
- [Cyfriflyfr mudo](ledger.md)
- [perfformiad](performance.md)
- [Adolygu cofnodion](issues.md)