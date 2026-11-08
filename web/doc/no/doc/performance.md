# Ytelse

Last ned `perf-latest` av `fkv_bench` fra fkv-depotet og kjør samme arbeidsmengde på en fast maskin.
Og sammenlign gjennomstrømning, latenskvantiler og topp RSS med en C++-grunnlinje.

Parameterkilden stemmer overens med fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++-grunnlinjen kjøres av orakelmiljøet `simple_bench` og sendes deretter ut i henhold til samme skjema og publiseres til
`perf-baseline` utgivelseselement. Sammenligningsresultatene skrives tilbake til lagerdokumentet, og nettstedet gjengir Markdown direkte.
Ingen JSON-datalag opprettholdes.

## Aktuelle resultater

skjema 2-grunnlinje for 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB-side,
`data_append_mode=false`, `direct_io=true`. Fast arbeidsbelastning:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| prosjekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Sett gjennomstrømning | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Få gjennomstrømning | 5,526 QPS | 7,293 QPS | 1.32x |
| Sett logisk båndbredde | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Få logisk båndbredde | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Topp RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Lagringsbruk | 269.0 MiB | 101.4 MiB | 0.38x |
| Lagring / nominelle data | 2.20x | 0.83x | 0.38x |

Å skrive 1 million oppføringer (10 000 batcher) tilsvarer
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, det vil si at data synkroniseres én gang per batch.
Rusttilleggsverdier: skanning `141,166 kv/s`, fullskanning `108,274 kv/s`, skriveforsterkning `2.57x`
(`proc_io_write` 329 MB / 128 MB logisk skriving), `4.86` skriv systemanrop per batch.
Lagringsforholdet er beregnet basert på 1M × 128B = 128 MB nominelle data; C++ utløser ikke GC/compact denne gangen,
Så den inneholder flere sider med historie. Det er ikke det samme kalibernummeret som "overlevelsesdataromforsterkning".
Rusts P99.9-halelatens er fortsatt dårligere enn C++ (sett 223,3 ms vs 86,0 ms, få 355,5 ms vs.
48,2 ms), den enkle grunnlinjen svinger mye, se den interne perf-rapporten for detaljer.

**Merk**: C++s `write-read` betyr at skrivetråden og lesetråden kjører samtidig, og Rust for øyeblikket
«Fullfør skriving, gjenåpn og les igjen», det er ingen skrivekonkurranse i lesestadiet. Derfor kan ikke multiplikatoren til `write-read` behandles som
Ren lesing / ren skrivekonklusjon; streng sammenligning bør se `scripts/perf_local.sh --symmetric`
To par resultater sendes ut: skrivebeskyttet og skrivebeskyttet. Enkeltdelte løpernummer brukes kun for regresjon
Trender, utgivelser vurderes ved å bruke medianen på `--runs`.