# eloqstore2fkv

eloqstore-dan fkv-ə köçürmə və uyğunluq yoxlama anbarı.

Əsas fkv deposu təmiz Rust olaraq qalır və müstəqil şəkildə qurulur; bu repozitoriyada C++ istinad tətbiqi oracle, iki istiqamətli bayt korpusu,
Mühasibat kitablarını, tarixi audit qeydlərini köçürün və CI-ni yoxlayın.

## məzmun

- C++ oracle: eloqstore format qatını əlaqələndirin və səhifə/indeks/daşqın/manifest/ yaradın
  korpusu sıxın.
- İki istiqamətli korpus: C++ yaz → Rust oxu, Rust yaz → C++ oxu.
- `fkv_compat`: Fkv anbarının format qutusuna əsaslanan pas tərəfi uyğunluq testi.
- `fkv_ledger`: Eloqstore-u skan edin, miqrasiya kitabçasını qeyd edin, korpus yaradın və yoxlayın.
- Performans müqayisəsi: Eyni iş yükü altında fkv və C++ bazalarını müqayisə edin.

## Giriş

- [Doğrulama prosesi](workflow.md)
- [C++ oracle](oracle.md)
- [Kitabı köçürün](ledger.md)
- [performans](performance.md)
- [Qeydləri nəzərdən keçirin](issues.md)