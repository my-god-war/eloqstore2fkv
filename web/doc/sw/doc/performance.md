# Utendaji

Pakua `perf-latest` ya `fkv_bench` kutoka kwa hazina ya fkv na utekeleze mzigo sawa wa kazi kwenye mashine isiyobadilika.
Na ulinganishe matokeo, quanti za muda wa kusubiri, na kilele cha RSS kwa C++ msingi.

Chanzo cha parameta ni sawa na fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Msingi wa C++ unaendeshwa na mazingira ya oracle `simple_bench` na kisha kutoa kulingana na schema sawa na kuchapishwa kwa
`perf-baseline` toa kipengee. Matokeo ya kulinganisha yameandikwa nyuma kwenye hati ya ghala, na tovuti inatoa Markdown moja kwa moja.
Hakuna safu ya data ya JSON inayodumishwa.

## Matokeo ya sasa

schema 2 msingi wa 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, ukurasa wa 4 KB,
`data_append_mode=false`, `direct_io=true`. Uzito wa kazi usiobadilika:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| mradi | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Weka matokeo | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Pata matokeo | 5,526 QPS | 7,293 QPS | 1.32x |
| Weka bandwidth ya kimantiki | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Pata kipimo data cha kimantiki | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Kilele cha RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Matumizi ya hifadhi | 269.0 MiB | 101.4 MiB | 0.38x |
| Hifadhi / data ya kawaida | 2.20x | 0.83x | 0.38x |

Kuandika maingizo milioni 1 (bati 10,000) inalingana na
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, yaani, data inasawazishwa mara moja kwa kila kundi.
Vipimo vya ziada vya kutu: Changanua `141,166 kv/s`, Changanua kikamilifu `108,274 kv/s`, Andika Ukuzaji `2.57x`
(`proc_io_write` 329 MB / 128 MB kimantiki huandika), `4.86` andika simu za mfumo kwa kila kundi.
Uwiano wa uhifadhi huhesabiwa kulingana na 1M × 128B = 128 MB data ya nominella; C++ haisababishi GC/compact wakati huu,
Kwa hivyo ina kurasa zaidi za historia. Sio nambari ya kiwango sawa na "ukuzaji wa nafasi ya data ya kuishi".
Ucheleweshaji wa mkia wa Rust wa P99.9 bado ni mbaya zaidi kuliko C++ (weka 223.3 ms dhidi ya 86.0 ms, pata 355.5 ms dhidi ya
48.2 ms), msingi mmoja hubadilikabadilika sana, tafadhali angalia ripoti ya utendaji wa ndani kwa maelezo.

**Kumbuka**: `write-read`C++'s inamaanisha kuwa uzi wa uandishi na uzi wa kusoma unaendeshwa kwa wakati mmoja, na Rust kwa sasa
"Kamilisha kuandika, fungua tena, na usome tena", hakuna mashindano ya uandishi katika hatua ya kusoma. Kwa hivyo kizidishi cha `write-read` hakiwezi kuchukuliwa kama
Hitimisho safi la kusoma / maandishi safi; ulinganisho mkali unapaswa kuona `scripts/perf_local.sh --symmetric`
Jozi mbili za matokeo ni pato: kuandika tu na kusoma tu. Nambari za mkimbiaji mmoja aliyeshirikiwa hutumiwa tu kwa urejeshaji
Mitindo, matoleo yanaamuliwa kwa kutumia wastani wa `--runs`.