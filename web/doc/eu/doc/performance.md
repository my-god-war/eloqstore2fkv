# Errendimendua

Deskargatu `perf-latest`-ren `fkv_bench` fkv biltegitik eta exekutatu lan-karga bera makina finko batean.
Eta alderatu transmisioa, latentzia kuantilak eta RSS gailurra C++ oinarrizko lerroarekin.

Parametroaren iturria fkv-rekin bat dator:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ oinarrizko lerroa oracle inguruneak `simple_bench` exekutatzen du eta, ondoren, eskema beraren arabera ateratzen da eta
`perf-baseline` askatu erreproduzigaia. Konparazioaren emaitzak biltegiko dokumentuan idazten dira, eta guneak Markdown zuzenean errendatzen du.
Ez da JSON datu-geruzarik mantentzen.