# eloqstore2fkv

Eloqstore'dan fkvga ko'chirish va muvofiqlikni tekshirish ombori.

Asosiy fkv ombori sof Rust bo'lib qoladi va mustaqil ravishda qurilgan; bu omborda C++ havolasini amalga oshirish oracle, ikki tomonlama bayt korpusi,
Hisob kitoblarini, tarixiy audit yozuvlarini ko'chiring va CIni tasdiqlang.

## mazmuni

- C++ oracle: eloqstore format qatlamini bog'lang va sahifa/indeks/toshib ketish/manifest/ yarating.
  korpusni siqish.
- Ikki tomonlama korpus: C++ yozish → Rust o‘qish, Rust yozish → C++ o‘qish.
- `fkv_compat`: fkv omborining format qutisiga tayanib, zang tomonida moslik testi.
- `fkv_ledger`: eloqstore-ni skanerlang, migratsiya kitobini yozib oling, korpusni yarating va tasdiqlang.
- Ishlashni taqqoslash: bir xil ish yuki ostida fkv va C++ tayanchlarini solishtiring.

## Kirish

- [Tasdiqlash jarayoni](workflow.md)
- [C++ oracle](oracle.md)
- [Ko'chirish kitobi](ledger.md)
- [ishlash](performance.md)
- [Yozuvlarni ko'rib chiqish](issues.md)