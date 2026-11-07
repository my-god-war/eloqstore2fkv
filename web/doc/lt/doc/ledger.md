# Perkelti knygą

`tool/fkv_ledger` Naudokite medžio prižiūrėtoją norėdami nuskaityti eloqstore, atvaizduoti ir patikrinti C++ simbolius ir fkv
Įrodymai rašomi į motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Didžiosios knygos duomenų bazė yra `.data/motedb.mote` ir nėra saugoma duomenų bazėje.

`verified` turi turėti patvirtinimą; `doc/progress.md` generuoja `report`.