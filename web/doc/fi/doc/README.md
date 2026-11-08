# eloqstore2fkv

Siirto- ja yhteensopivuusvarmistusvarasto eloqstoresta fkv:hen.

Fkv-päävarasto pysyy puhtaana ruosteena ja rakennetaan itsenäisesti; tämä arkisto säilyttää C++-viitetoteutusoraakkelin, kaksisuuntaisen tavukorpuksen,
Siirrä pääkirjat, historialliset tarkastustietueet ja tarkista CI.

## sisältöä

- C++ Oracle: linkitä eloqstore-muotokerros ja luo sivu/indeksi/ylivuoto/manifest/
  pakkaa korpus.
- Kaksisuuntainen korpus: C++ kirjoitus → Rust read, Rust write → C++ read.
- `fkv_compat`: Ruostepuolen yhteensopivuustesti fkv-varaston muotolaatikon perusteella.
- `fkv_ledger`: Tarkista eloqstore, tallenna siirtokirja, luo ja tarkista korpus.
- Suorituskyvyn vertailu: Vertaa fkv- ja C++-perusarvoja samalla työmäärällä.

## Sisäänkäynti

- [Vahvistusprosessi](workflow.md)
- [C++ oracle](oracle.md)
- [Siirrä kirjanpito](ledger.md)
- [suorituskykyä](performance.md)
- [Tarkista tietueet](issues.md)