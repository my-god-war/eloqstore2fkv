---
date: 2026-11-06
summary: Əsas səhifə performansının müqayisəsi, qiymətləndirmə göstəricisinin genişləndirilməsi, GitHub Səhifələri və onlayn çoxdilli bloqlar
---

# 2026-11-06: Əsas səhifə performans cədvəli, göstərici genişləndirilməsi və Səhifələr

Bugünkü yeniləmə performansın nəzərdən keçirilməsinə və saytın işə salınmasına yönəlib.

## Əsas səhifə performansının müqayisəsi

- Əsas səhifə performans prioritetinə dəyişdirilir və C++/Rust müqayisə cədvəli birbaşa birinci ekranda göstərilir.
- Üç yeni göstərici dəsti əlavə edildi: məntiqi bant genişliyi, P99.9 quyruq gecikməsi və məkan gücləndirilməsi.
- Əlavə edilmiş Rust işləmə təfərrüatları: Scan, Fullscan, yazma gücləndirilməsi, yazma sistemi zənglərinin sayı və hər partiya üçün məlumat sinxronizasiyası.
- Açıqca `write-read`-ni qarışıq kalibr kimi qeyd edin və performans sənədlərinə keçid edin.

Cari eyni maşın bazası:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Pik RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Saxlama istifadəsi | 269.0 MiB | 101.4 MiB | 0.38x |

## Qiymətləndirmə göstəriciləri

RocksDB, LevelDB, YCSB, WiredTiger və digər meyarların ümumi nəticələrinə istinad edərək əlavə etdim:

- Məntiqi oxuma və yazma bant genişliyi MiB/s;
- P99.9 quyruq gecikməsi;
- Kosmosun gücləndirilməsi, yazı gücləndirilməsi;
- Hər partiyada sistem zənglərinin və məlumatların sinxronizasiyasının sayı;
- Scan / Fullscan ötürmə qabiliyyəti;
- CPU, səhifə xətaları, kontekst keçidləri və proses IO resurs yan arabası tərəfindən toplanmağa davam edir.

Eyni zamanda təqaüdə çıxdı`gxhash_4096B_gib_s`. Bu, KV mühərrik yolunda olmayan və C++ analoqu olmayan 4 KiB tək bloklu hash mikrobençmarkıdır.

## Düzgünlük və mühəndislik düzəlişləri

- `--rng cpp` oxu/skan əsas boşluğunu düzəldin: oxunmaların 75%-nin yazılmamış düymələrə düşməməsi üçün oxunma aşkarlanması artıq `key_interval` ilə vurulmur.
- Eyni məzmunlu manifestlərin macOS `openrsync` səhv hesablanması nəticəsində yaranan sabit gözləmə rejimində təkrar cəhdlər.
- QEMU virtual disklərinin yerli SSD-lər kimi yanlış qiymətləndirildiyi hardware qapısı problemi həll edildi.

## Saytlar və Nəşriyyat

- GitHub Səhifələri aktivləşdirildi: <https://my-god-war.github.io/eloqstore2fkv/>
- Saytın marşrutlaşdırılması `/eloqstore2fkv/` alt yolları dəstəkləyir.
- Bunun əvəzinə sənəd `raw.githubusercontent.com`-dən oxunur və jsDelivr yolunda itkin `web/` səbəb olduğu 404 düzəldilir.
- Bloq naviqasiyası və `bun run tran` çoxdilli tərcümə inteqrasiya olunub; bu məqalə avtomatik olaraq Çin mənbəsindən digər dillərə tərcümə olunur.

Sonrakı dəyişikliklər qeydləri uyğunluq, performans və alətlər silsiləsi dəyişikliklərini sənədləşdirməyə davam edəcək.