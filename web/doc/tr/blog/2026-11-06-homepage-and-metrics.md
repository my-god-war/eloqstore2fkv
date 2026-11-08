---
date: 2026-11-06
summary: Ana sayfa performans karşılaştırması, değerlendirme göstergesi genişletme, GitHub Sayfaları ve çevrimiçi çok dilli bloglar
---

# 2026-11-06: Ana sayfa performans tablosu, gösterge genişletmesi ve Sayfalar

Bugünkü güncelleme performans incelemelerine ve site lansmanlarına odaklanıyor.

## Ana sayfa performans karşılaştırması

- Ana sayfa performans önceliğine dönüştürülür ve C++/Rust karşılaştırma tablosu doğrudan ilk ekranda görüntülenir.
- Üç yeni gösterge seti eklendi: mantıksal bant genişliği, P99.9 kuyruk gecikmesi ve uzaysal amplifikasyon.
- Rust çalıştırma ayrıntıları eklendi: Tarama, Tam Tarama, yazma güçlendirmesi, yazma sistemi çağrılarının sayısı ve grup başına veri senkronizasyonu.
- Açıkça `write-read`'i karışık kalibre olarak işaretleyin ve performans belgelerine bağlantı verin.

Mevcut aynı makine temel çizgisi:

| dizin | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS'nin zirvesi | 175.6 MiB | 145.5 MiB | 0.83x |
| Depolama kullanımı | 269.0 MiB | 101.4 MiB | 0.38x |

## Değerlendirme göstergeleri

RocksDB, LevelDB, YCSB, WiredTiger ve diğer kriterlerin ortak çıktılarına atıfta bulunarak şunları ekledim:

- Mantıksal okuma ve yazma bant genişliği MiB/s;
- P99.9 kuyruk gecikmesi;
- Uzay amplifikasyonu, yazı amplifikasyonu;
- Her grupta yazma sistemi çağrılarının ve veri senkronizasyonunun sayısı;
- Tarama / Tam tarama verimi;
- CPU, sayfa hataları, içerik anahtarları ve işlem GÇ'si kaynak sepeti tarafından toplanmaya devam eder.

Aynı zamanda emekli oldu`gxhash_4096B_gib_s`. KV motor yolunda olmayan ve C++ karşılığı olmayan, 4 KiB'lik tek bloklu bir karma mikro kıyaslamadır.

## Doğruluk ve mühendislik düzeltmeleri

- `--rng cpp` okuma/tarama anahtar alanını düzeltin: Okumaların %75'inin yazılmamış anahtarlara düşmesini önlemek için okuma algılaması artık `key_interval` ile çarpılmıyor.
- Aynı içeriğe sahip manifestlerin macOS `openrsync` yanlış değerlendirilmesinden kaynaklanan bekleme modundaki yeniden denemeler düzeltildi.
- QEMU sanal disklerinin yerel SSD olarak yanlış değerlendirilmesine neden olan donanım kapısı sorunu düzeltildi.

## Siteler ve Yayıncılık

- GitHub Sayfaları etkinleştirildi: <https://my-god-war.github.io/eloqstore2fkv/>
- Site yönlendirmesi `/eloqstore2fkv/` alt yollarını destekler.
- Belge bunun yerine `raw.githubusercontent.com` adresinden okunur ve jsDelivr yolundaki eksik `web/` nedeniyle oluşan 404 düzeltilir.
- Blogda gezinme ve `bun run tran` çoklu dil çevirisi entegre edilmiştir; Bu makale Çince kaynaktan otomatik olarak diğer dillere çevrilmiştir.

Sonraki değişiklik günlükleri uyumluluk, performans ve araç zinciri değişikliklerini belgelemeye devam edecektir.