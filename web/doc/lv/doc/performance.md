# Veiktspēja

Lejupielādējiet `perf-latest` no `fkv_bench` no fkv repozitorija un palaidiet to pašu darba slodzi fiksētā datorā.
Un salīdziniet caurlaidspēju, latentuma kvantiles un maksimālo RSS ar C++ bāzes līniju.

Parametra avots atbilst fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ bāzes līniju palaiž Oracle vide `simple_bench` un pēc tam izvada saskaņā ar to pašu shēmu un publicē
`perf-baseline` atbrīvot līdzekli. Salīdzināšanas rezultāti tiek ierakstīti atpakaļ noliktavas dokumentā, un vietne tieši atveido Markdown.
JSON datu slānis netiek uzturēts.

## Pašreizējie rezultāti

2. shēmas pamatlīnija 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB lapa,
`data_append_mode=false`, `direct_io=true`. Fiksēta darba slodze:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projektu | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Ielieciet caurlaidspēju | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Iegūstiet caurlaidspēju | 5,526 QPS | 7,293 QPS | 1.32x |
| Ievietojiet loģisko joslas platumu | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Iegūstiet loģisko joslas platumu | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Maksimālais RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Krātuves izmantošana | 269.0 MiB | 101.4 MiB | 0.38x |
| Uzglabāšana / nominālie dati | 2.20x | 0.83x | 0.38x |

1 miljona ierakstu (10 000 partiju) rakstīšana atbilst
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, tas ir, dati tiek sinhronizēti vienreiz vienā partijā.
Papildu rūsas metrika: skenēšana `141,166 kv/s`, pilna skenēšana `108,274 kv/s`, rakstīšanas pastiprināšana `2.57x`
(`proc_io_write` 329 MB / 128 MB loģiski ieraksti), `4.86` rakstīt sistēmas izsaukumus pa partiju.
Uzglabāšanas koeficients tiek aprēķināts, pamatojoties uz 1M × 128B = 128 MB nominālajiem datiem; C++ šoreiz neaktivizē GC/compact,
Tātad tajā ir vairāk vēstures lappušu. Tas nav tāds pats kalibra skaitlis kā "izdzīvošanas datu telpas pastiprināšana".
Rust's P99.9 astes latentums joprojām ir sliktāks nekā C++ (ielieciet 223,3 ms pret 86,0 ms, iegūstiet 355,5 ms vs.
48,2 ms), viena bāzes līnija ļoti svārstās. Lai iegūtu sīkāku informāciju, lūdzu, skatiet iekšējo darbības pārskatu.

**Piezīme**: C++ `write-read` nozīmē, ka rakstīšanas pavediens un lasīšanas pavediens darbojas vienlaikus un Rust pašlaik
"Pabeigt rakstīšanu, atveriet vēlreiz un lasiet vēlreiz", lasīšanas posmā nav rakstīšanas konkursa. Tāpēc reizinātāju `write-read` nevar uzskatīt par
Tīras lasīšanas / tīras rakstīšanas secinājums; stingrai salīdzināšanai vajadzētu redzēt `scripts/perf_local.sh --symmetric`
Tiek izvadīti divi rezultātu pāri: tikai rakstāmie un tikai lasāmie. Viena koplietota skrējēja numuri tiek izmantoti tikai regresijai
Tendences, izlaidumi tiek vērtēti, izmantojot vidējo `--runs`.