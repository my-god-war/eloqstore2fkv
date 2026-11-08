# eloqstore2fkv

Eloqstore'dan fkv'ye geçiş ve uyumluluk doğrulama deposu.

Ana fkv deposu saf Rust olarak kalır ve bağımsız olarak inşa edilir; bu depo, C++ referans uygulama kehanetini, çift yönlü bayt külliyatını korur,
Defterleri ve geçmiş denetim kayıtlarını taşıyın ve CI'yi doğrulayın.

## içerik

- C++ oracle: eloqstore format katmanını bağlayın ve sayfa/dizin/taşma/bildirim/ oluşturun
  korpusu sıkıştırın.
- Çift yönlü derlem: C++ yazma → Pas okuma, Pas yazma → C++ okuma.
- `fkv_compat`: Fkv deposunun format kasasına dayalı olarak pas tarafı uyumluluk testi.
- `fkv_ledger`: Eloqstore'u tarayın, geçiş defterini kaydedin, derlem oluşturun ve doğrulayın.
- Performans karşılaştırması: Aynı iş yükü altında fkv ve C++ temellerini karşılaştırın.

## Giriş

- [Doğrulama süreci](workflow.md)
- [C++ oracle](oracle.md)
- [Defteri taşı](ledger.md)
- [performans](performance.md)
- [Kayıtları inceleyin](issues.md)