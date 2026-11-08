# Unumdorlik

Fkv omboridan `fkv_bench`ning `perf-latest` ni yuklab oling va bir xil ish yukini sobit mashinada bajaring.
Va o'tkazish qobiliyati, kechikish kvantlari va eng yuqori RSS-ni C++ bazasi bilan solishtiring.

Parametr manbai fkv bilan mos keladi:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ bazasi oracle muhiti `simple_bench` tomonidan boshqariladi va keyin bir xil sxema bo'yicha chiqariladi va nashr etiladi.
`perf-baseline` aktivni chiqarish. Taqqoslash natijalari ombor hujjatiga qayta yoziladi va sayt to'g'ridan-to'g'ri Markdownni taqdim etadi.
Hech qanday JSON ma'lumotlar qatlami saqlanmaydi.

## Joriy natijalar

2-sxema asosiy 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB sahifa,
`data_append_mode=false`, `direct_io=true`. Ruxsat etilgan ish yuki:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| loyiha | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| O'tkazish qobiliyati | 16,866 kv/s | 21,377 kv/s | 1.27x |
| O'tkazish qobiliyatini oling | 5,526 QPS | 7,293 QPS | 1.32x |
| Mantiqiy tarmoqli kengligi qo'ying | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Mantiqiy tarmoqli kengligi oling | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Eng yuqori RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Saqlashdan foydalanish | 269.0 MiB | 101.4 MiB | 0.38x |
| Saqlash / nominal ma'lumotlar | 2.20x | 0.83x | 0.38x |

1 million yozuvni yozish (10 000 partiya) mos keladi
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, ya'ni ma'lumotlar to'plamda bir marta sinxronlashtiriladi.
Zanglashning qoʻshimcha koʻrsatkichlari: skanerlash `141,166 kv/s`, toʻliq skanerlash `108,274 kv/s`, yozishni kuchaytirish `2.57x`
(`proc_io_write` 329 MB / 128 MB mantiqiy yozish), `4.86` har bir partiya uchun tizim chaqiruvlarini yozish.
Saqlash nisbati 1M × 128B = 128 MB nominal ma'lumotlar asosida hisoblanadi; C++ bu safar GC/ixchamni ishga tushirmaydi,
Shunday qilib, u tarixning ko'proq sahifalarini o'z ichiga oladi. Bu "omon qolish ma'lumotlari maydonini kuchaytirish" bilan bir xil kalibrli raqam emas.
Rust-ning P99.9 dumining kechikishi hali ham C++ dan yomonroq (223,3 ms va 86,0 ms, 355,5 ms va boshqalarni oling).
48,2 ms), yagona bazaviy chiziq juda o'zgarib turadi, batafsil ma'lumot uchun ichki perf hisobotiga qarang.

**Eslatma**: C++ ning `write-read` yozuvi va o'qish chizig'i bir vaqtning o'zida ishlayotganini bildiradi va hozirda Rust
"Yozishni tugat, qayta oching va qayta o'qing", o'qish bosqichida yozish tanlovi yo'q. Shuning uchun `write-read` ko'paytuvchisi sifatida ko'rib chiqilishi mumkin emas
Sof o'qish / sof yozish xulosasi; qat'iy taqqoslash uchun `scripts/perf_local.sh --symmetric` ko'rish kerak
Ikki juft natijalar chiqariladi: faqat yozish va faqat o'qish uchun. Yagona umumiy yuguruvchi raqamlari faqat regressiya uchun ishlatiladi
Trendlar, relizlar `--runs` medianasi yordamida baholanadi.