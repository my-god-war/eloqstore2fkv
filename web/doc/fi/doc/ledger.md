# Siirrä kirjanpito

`tool/fkv_ledger` Käytä tree-sitteriä eloqstoren skannaamiseen, kartoittamiseen ja tarkistamiseen C++-symbolit ja fkv
Todisteet kirjoitetaan motedb:lle.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Reskontratietokanta on osoitteessa `.data/motedb.mote`, eikä sitä ole tallennettu tietokantaan.

`verified` on oltava hyväksytty todiste; `doc/progress.md` on luonut `report`.