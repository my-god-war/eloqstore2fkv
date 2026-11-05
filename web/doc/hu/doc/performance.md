# Teljesítmény

Töltse le a(z) `perf-latest` / `fkv_bench` `fkv_bench` elemet az fkv tárolóból, és futtassa ugyanazt a terhelést egy rögzített gépen.
És hasonlítsa össze az átviteli sebességet, a késleltetési kvantisokat és a csúcs RSS-t a C++ alapvonallal.

A paraméter forrása összhangban van az fkv-vel:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

A C++ alapvonalat az oracle környezet `simple_bench` futtatja, majd ugyanazon séma szerint adja ki, és közzéteszi
`perf-baseline` kiadási eszköz. Az összehasonlítási eredmények visszaírásra kerülnek a raktári dokumentumba, és a webhely közvetlenül megjeleníti a Markdown-t.
Nincs karbantartott JSON-adatréteg.