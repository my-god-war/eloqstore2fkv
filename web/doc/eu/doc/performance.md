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

## Egungo emaitzak

2026-11-06T04:25Zrako 2. eskema,`github-ubuntu-latest-x86_64`, 4 KiB orrialdea,
`data_append_mode=false`, `direct_io=true`. Lan karga finkoa:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proiektua | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Jarri errendimendua | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Lortu errendimendua | 5,526 QPS | 7,293 QPS | 1.32x |
| Jarri banda-zabalera logikoa | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Lortu banda-zabalera logikoa | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS gailurra | 175.6 MiB | 145.5 MiB | 0.83x |
| Biltegiratze erabilera | 269.0 MiB | 101.4 MiB | 0.38x |
| Biltegiratzea / datu nominalak | 2.20x | 0.83x | 0.38x |

Milioi 1 sarrera (10.000 lote) idaztea dagokio
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, hau da, datuak lote bakoitzeko behin sinkronizatzen dira.
Herdoildu neurri osagarriak: eskaneatu `141,166 kv/s`, Fullscan `108,274 kv/s`, Idatzi anplifikazioa `2.57x`
(`proc_io_write` 329 MB / 128 MB idazketa logikoak), `4.86` sistema-deiak idatzi lote bakoitzeko.
Biltegiratze-erlazioa 1M × 128B = 128 MB datu nominaletan oinarrituta kalkulatzen da; C++-k ez du GC/compact abiarazten oraingoan,
Beraz, historiako orrialde gehiago ditu. Ez da "biziraupen-datuen espazioaren anplifikazio" kalibre berekoa.
Rust-en P99.9 buztaneko latentzia C++ baino okerragoa da oraindik (jarri 223,3 ms vs 86,0 ms, lortu 355,5 ms vs.
48,2 ms), oinarri-lerro bakarrak asko aldatzen du; mesedez, ikusi barne perf txostena xehetasunetarako.

**Oharra**: C++-ren `write-read`k esan nahi du idazteko haria eta irakurtzeko haria aldi berean exekutatzen direla, eta Rust une honetan
"Osatu idazketa, berriro ireki eta berriro irakurri", ez dago idazketa lehiaketarik irakurketa fasean. Beraz, `write-read`-ren biderkatzailea ezin da horrela tratatu
Irakurketa hutsa / idazketa hutsa ondorioa; konparaketa zorrotzak `scripts/perf_local.sh --symmetric` ikusi beharko luke
Bi emaitza pare ateratzen dira: idazteko soilik eta irakurtzeko soilik. Partekatutako korrikalari bakarreko zenbakiak erregresiorako soilik erabiltzen dira
Joera, argitalpenak `--runs`-ren mediana erabiliz epaitzen dira.