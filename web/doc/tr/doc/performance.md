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