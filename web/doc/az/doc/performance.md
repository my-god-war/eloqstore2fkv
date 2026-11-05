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