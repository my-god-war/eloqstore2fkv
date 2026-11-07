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

## Rezultate curente

schema 2 de referință pentru 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, pagină de 4 KiB,
`data_append_mode=false`, `direct_io=true`. Volumul de lucru fix:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| proiect | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Pune debit | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Obține debit | 5,526 QPS | 7,293 QPS | 1.32x |
| Pune lățimea de bandă logică | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Obțineți lățime de bandă logică | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Vârf RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Utilizarea depozitării | 269.0 MiB | 101.4 MiB | 0.38x |
| Stocare / date nominale | 2.20x | 0.83x | 0.38x |

Scrierea a 1 milion de intrări (10.000 de loturi) corespunde
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, adică datele sunt sincronizate o dată pe lot.
Valori suplimentare pentru rugină: Scanare `141,166 kv/s`, Scanare completă `108,274 kv/s`, Scriere Amplificare `2.57x`
(`proc_io_write` 329 MB / 128 MB scrieri logice), `4.86` scrie apeluri de sistem per lot.
Raportul de stocare este calculat pe baza 1M × 128B = 128 MB de date nominale; C++ nu declanșează GC/compact de data aceasta,
Deci conține mai multe pagini de istorie. Nu este același număr de calibru cu „amplificarea spațiului de date de supraviețuire”.
Latența de coadă a lui Rust P99.9 este încă mai slabă decât C++ (puneți 223,3 ms vs 86,0 ms, obțineți 355,5 ms vs.
48,2 ms), linia de bază unică fluctuează foarte mult, vă rugăm să consultați raportul de performanță intern pentru detalii.

**Notă**: `write-read` C++ înseamnă că firul de scriere și firul de citire rulează în același timp, iar Rust în prezent
„Scrieți complet, redeschideți și citiți din nou”, nu există concurs de scriere în etapa de lectură. Prin urmare, multiplicatorul lui `write-read` nu poate fi tratat ca
Lectură pură / concluzie pur scris; comparația strictă ar trebui să vadă `scripts/perf_local.sh --symmetric`
Sunt ieșite două perechi de rezultate: numai pentru scriere și numai pentru citire. Numerele unice ale alergătorului partajate sunt folosite doar pentru regresie
Tendințele, lansările sunt evaluate folosind mediana `--runs`.