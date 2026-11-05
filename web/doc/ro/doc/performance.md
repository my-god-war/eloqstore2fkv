# Performanță

Descărcați `perf-latest` din `fkv_bench` din depozitul fkv și rulați aceeași sarcină de lucru pe o mașină fixă.
Și comparați debitul, cuantilele de latență și RSS de vârf cu o linie de bază C++.

Sursa parametrului este în concordanță cu fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Linia de bază C++ este rulată de mediul oracle `simple_bench` și apoi emite în conformitate cu aceeași schemă și publicată la
`perf-baseline` lansează materialul. Rezultatele comparației sunt scrise înapoi în documentul de depozit, iar site-ul redă direct Markdown.
Nu este menținut niciun strat de date JSON.