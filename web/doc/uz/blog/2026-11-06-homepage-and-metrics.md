---
date: 2026-11-06
summary: Bosh sahifa samaradorligini taqqoslash, baholash ko'rsatkichini kengaytirish, GitHub sahifalari va onlayn ko'p tilli bloglar
---

# 2026-11-06: Bosh sahifa ishlash jadvali, indikatorni kengaytirish va Sahifalar

Bugungi yangilanish ish faoliyatini baholash va saytni ishga tushirishga qaratilgan.

## Bosh sahifa samaradorligini taqqoslash

- Bosh sahifa ishlash ustuvorligiga o'zgartiriladi va C++/Rust taqqoslash jadvali bevosita birinchi ekranda ko'rsatiladi.
- Uchta yangi ko'rsatkichlar to'plami qo'shildi: mantiqiy tarmoqli kengligi, P99.9 quyruq kechikishi va fazoviy kuchaytirish.
- Qo'shilgan Rust ish tafsilotlari: Skanerlash, To'liq skanerlash, yozishni kuchaytirish, yozish tizimi qo'ng'iroqlari soni va har partiya uchun ma'lumotlarni sinxronlashtirish.
- `write-read`ni aralash kalibrli deb aniq belgilang va ishlash hujjatlariga havola qiling.

Ayni mashinaning joriy bazasi:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Eng yuqori RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Saqlashdan foydalanish | 269.0 MiB | 101.4 MiB | 0.38x |

## Baholash ko'rsatkichlari

RocksDB, LevelDB, YCSB, WiredTiger va boshqa mezonlarning umumiy natijalariga ishora qilib, men qo'shimcha qildim:

- Mantiqiy o'qish va yozish tarmoqli kengligi MiB/s;
- P99.9 quyruq kechikishi;
- Fazoni kuchaytirish, yozishni kuchaytirish;
- Har bir partiyada tizim qo'ng'iroqlari va ma'lumotlarni sinxronlashtirish soni;
- Scan / Fullscan o'tkazish qobiliyati;
- CPU, sahifadagi nosozliklar, kontekstli kalitlar va jarayon IO resurs yordamchisi tomonidan to'planishi davom etmoqda.

Bir vaqtning o'zida nafaqaga chiqqan`gxhash_4096B_gib_s`. Bu KV dvigatel yo'lida bo'lmagan va C++ hamkasbiga ega bo'lmagan 4 KiB yagona blokli hash mikrobenchmarkidir.

## To'g'rilik va muhandislik tuzatishlari

- `--rng cpp` o'qish/skanerlash kalit bo'shlig'ini to'g'irlang: o'qishni aniqlash endi `key_interval` ga ko'paytirilmaydi, o'qishlarning 75% yozilmagan kalitlarga tushib qolmasligi uchun.
- MacOS `openrsync` bir xil tarkibga ega manifestlarni noto'g'ri baholaganligi sababli kutish rejimidagi qayta urinishlar tuzatildi.
- QEMU virtual disklari mahalliy SSD sifatida noto'g'ri baholangan apparat shlyuzi muammosi tuzatildi.

## Saytlar va nashriyot

- GitHub sahifalari yoqilgan: <https://my-god-war.github.io/eloqstore2fkv/>
- Sayt marshruti `/eloqstore2fkv/` pastki yo'llarni qo'llab-quvvatlaydi.
- Hujjat o'rniga `raw.githubusercontent.com` dan o'qiladi va jsDelivr yo'lida `web/` etishmayotganligi sababli yuzaga kelgan 404 belgilandi.
- Blog navigatsiyasi va `bun run tran` ko'p tilli tarjima birlashtirilgan; ushbu maqola avtomatik ravishda xitoycha manbadan boshqa tillarga tarjima qilinadi.

Keyingi o'zgarishlar jurnallari muvofiqlik, ishlash va asboblar zanjiridagi o'zgarishlarni hujjatlashtirishda davom etadi.