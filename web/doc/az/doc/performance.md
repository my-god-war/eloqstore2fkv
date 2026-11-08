# Performans

Fkv deposundan `fkv_bench` faylın `perf-latest`-ni endirin və sabit maşında eyni iş yükünü işə salın.
Və ötürmə qabiliyyətini, gecikmə kvantillərini və pik RSS-ni C++ bazası ilə müqayisə edin.

Parametr mənbəyi fkv ilə uyğundur:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ baza xətti `simple_bench` Oracle mühiti tərəfindən idarə olunur və sonra eyni sxemə uyğun olaraq çıxarılır və burada dərc olunur.
`perf-baseline` aktivi buraxın. Müqayisə nəticələri yenidən anbar sənədinə yazılır və sayt birbaşa Markdown təqdim edir.
Heç bir JSON məlumat qatı saxlanılmır.

## Cari nəticələr

2026-11-06T04:25Z üçün sxem 2 əsas xətt,`github-ubuntu-latest-x86_64`, 4 KiB səhifə,
`data_append_mode=false`, `direct_io=true`. Sabit iş yükü:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| layihə | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Ötürmə qabiliyyətini qoyun | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Ötürmə qabiliyyəti əldə edin | 5,526 QPS | 7,293 QPS | 1.32x |
| Məntiqi bant genişliyi qoyun | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Məntiqi bant genişliyi əldə edin | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Pik RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Saxlama istifadəsi | 269.0 MiB | 101.4 MiB | 0.38x |
| Saxlama / nominal məlumat | 2.20x | 0.83x | 0.38x |

1 milyon girişin yazılması (10.000 partiya) uyğun gəlir
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, yəni verilənlər hər topluda bir dəfə sinxronlaşdırılır.
Pas əlavə ölçülər: Skan `141,166 kv/s`, Tam skan `108,274 kv/s`, Yazı Gücləndirilməsi `2.57x`
(`proc_io_write` 329 MB / 128 MB məntiqi yazılar), `4.86` hər toplu üçün sistem zənglərini yazın.
Saxlama nisbəti 1M × 128B = 128 MB nominal məlumat əsasında hesablanır; C++ bu dəfə GC/kompaktı işə salmır,
Beləliklə, daha çox tarixin səhifələrini ehtiva edir. Bu, "sağ qalan məlumat məkanının gücləndirilməsi" ilə eyni çaplı nömrə deyil.
Rust-un P99.9 quyruq gecikməsi hələ də C++-dan daha pisdir (223.3 ms-ə qarşı 86.0 ms qoyun, 355.5 ms-ə qarşı alın.
48,2 ms), vahid əsas xətt çox dəyişir, təfərrüatlar üçün daxili perf hesabatına baxın.

**Qeyd**: C++-nın `write-read` o deməkdir ki, yazı xətti və oxuma xətti eyni vaxtda işləyir və hazırda Rust
“Yazını tamamla, yenidən aç və yenidən oxu” oxu mərhələsində yazı müsabiqəsi yoxdur. Buna görə də `write-read` çarpanı kimi qəbul edilə bilməz
Təmiz oxu / təmiz yazı nəticəsi; ciddi müqayisə `scripts/perf_local.sh --symmetric` görməlidir
İki cüt nəticə çıxarılır: yalnız yazmaq üçün və yalnız oxumaq üçün. Tək paylaşılan qaçış nömrələri yalnız reqressiya üçün istifadə olunur
Trendlər, buraxılışlar `--runs` medianından istifadə edilməklə qiymətləndirilir.