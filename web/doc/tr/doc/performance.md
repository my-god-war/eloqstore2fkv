# Performans

Fkv deposundan `perf-latest` / `fkv_bench` dosyasını indirin ve aynı iş yükünü sabit bir makinede çalıştırın.
Ayrıca aktarım hızını, gecikme yüzdelik dilimlerini ve en yüksek RSS'yi bir C++ temel çizgisiyle karşılaştırın.

Parametre kaynağı fkv ile tutarlıdır:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ temel çizgisi `simple_bench` oracle ortamı tarafından çalıştırılır ve ardından aynı şemaya göre çıktılanır ve şuraya yayınlanır:
`perf-baseline` öğeyi serbest bırak. Karşılaştırma sonuçları depo belgesine geri yazılır ve site doğrudan Markdown'ı oluşturur.
Hiçbir JSON veri katmanı korunmaz.

## Mevcut sonuçlar

2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB sayfası için şema 2 temel çizgisi,
`data_append_mode=false`, `direct_io=true`. Sabit iş yükü:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proje | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Verimi koy | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Verim elde edin | 5,526 QPS | 7,293 QPS | 1.32x |
| Mantıksal bant genişliğini koyun | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Mantıksal bant genişliği elde edin | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS'nin zirvesi | 175.6 MiB | 145.5 MiB | 0.83x |
| Depolama kullanımı | 269.0 MiB | 101.4 MiB | 0.38x |
| Depolama / nominal veriler | 2.20x | 0.83x | 0.38x |

1 milyon giriş (10.000 parti) yazmak şuna karşılık gelir:
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, yani veriler toplu iş başına bir kez senkronize edilir.
Rust ek ölçümleri: Tarama `141,166 kv/s`, Tam Tarama `108,274 kv/s`, Yazma Yükseltmesi `2.57x`
(`proc_io_write` 329 MB / 128 MB mantıksal yazma), toplu iş başına `4.86` yazma sistem çağrıları.
Depolama oranı 1M × 128B = 128 MB nominal veriye göre hesaplanır; C++ bu sefer GC/compact'ı tetiklemiyor,
Yani daha fazla tarih sayfası içeriyor. "Hayatta kalma veri alanı amplifikasyonu" ile aynı kalibre numarası değildir.
Rust'un P99.9 kuyruk gecikmesi hala C++'dan daha kötü (223,3 ms'ye karşılık 86,0 ms, 355,5 ms'ye karşılık
48,2 ms), tek taban çizgisi büyük ölçüde dalgalanıyor; ayrıntılar için lütfen dahili performans raporuna bakın.

**Not**: C++'ın `write-read` özelliği, yazma ve okuma iş parçacığının aynı anda çalıştığı ve Rust'un şu anda çalıştığı anlamına gelir
"Yazmayı tamamla, tekrar aç, tekrar oku", okuma aşamasında yazma yarışması yapılmaz. Bu nedenle `write-read` çarpanı şu şekilde değerlendirilemez:
Saf okuma / saf yazma sonucu; kesin karşılaştırma şunu görmelidir: `scripts/perf_local.sh --symmetric`
İki çift sonucun çıktısı alınır: salt okunur ve salt okunur. Tek paylaşılan koşucu sayıları yalnızca regresyon için kullanılır
Eğilimler ve yayınlar, `--runs` ortalama değeri kullanılarak değerlendirilir.