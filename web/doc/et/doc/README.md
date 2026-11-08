# eloqstore2fkv

Migratsiooni ja ühilduvuse kontrolli ladu eloqstore'ist fkv-sse.

Peamine fkv hoidla jääb puhtaks rooste ja ehitatakse iseseisvalt; see hoidla säilitab C++ viiterakenduse oraakli, kahesuunalise baidikorpuse,
Viige üle pearaamatud, ajaloolised auditikirjed ja kontrollige CI-d.

## sisu

- C++ Oracle: linkige eloqstore'i vormingukiht ja genereerige leht/indeks/ülevool/manifest/
  korpust kokku suruma.
- Kahesuunaline korpus: C++ kirjutamine → Rust read, Rust write → C++ read.
- `fkv_compat`: roostepoolne ühilduvuse test, tuginedes fkv lao vormingukastile.
- `fkv_ledger`: eloqstore'i skannimine, migratsioonireskontra salvestamine, korpuse genereerimine ja kontrollimine.
- Toimivuse võrdlus: võrrelge fkv ja C++ baasväärtusi sama töökoormuse korral.

## Sissepääs

- [Kinnitusprotsess](workflow.md)
- [C++ oracle](oracle.md)
- [Pearaamatu migreerimine](ledger.md)
- [esitus](performance.md)
- [Vaadake kirjed üle](issues.md)